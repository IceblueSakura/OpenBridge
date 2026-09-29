//! Offline diagnostic: `cargo run --example replay_chat -- PROVIDER CAPTURE.json|sse`.
//! Reads only the supplied bounded capture; never loads credentials or opens sockets.
use openbridge::{
    adapter::{Adapter, Dialect},
    execution::{Attempt, ResponseDelivery},
    lowering::generation::GenerationRepresentationContract,
    protocol::openai::sse::Obfuscation,
    protocol::openai::{Profile, sse::SseLimits},
    semantic::{
        context::StreamOptions,
        value::{Presence, ReplayOrigin},
    },
};
use std::{fs::File, io::Read};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("expected provider and capture path".into());
    }
    let dialect = match args[1].as_str() {
        "longcat" => Dialect::LongCat,
        "nvidia" => Dialect::Nvidia,
        "bailian" => Dialect::Bailian,
        "kimi" => Dialect::Kimi,
        "zhipu" => Dialect::Zhipu,
        _ => return Err("unknown diagnostic provider".into()),
    };
    let adapter = Adapter::new(
        Profile::Chat,
        dialect,
        Some(ReplayOrigin::new("offline-diagnostic")?),
    );
    let mut bytes = Vec::new();
    File::open(&args[2])?
        .take((2 << 20) + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 2 << 20 {
        return Err("capture budget exceeded".into());
    }
    let stream = args[2].ends_with(".sse");
    let mut attempt = Attempt::new(adapter, 2 << 20, SseLimits::default());
    attempt.begin(
        200,
        if stream {
            "text/event-stream"
        } else {
            "application/json"
        },
    )?;
    let mut delivery = ResponseDelivery::new(
        Adapter::new(
            Profile::Chat,
            Dialect::OpenBridge,
            Some(ReplayOrigin::new("offline-diagnostic")?),
        ),
        GenerationRepresentationContract::full(),
        "public-model",
        SseLimits::default(),
        StreamOptions {
            include_usage: Presence::Value(true),
            include_obfuscation: Presence::Value(false),
        },
        Obfuscation::Disabled,
    );
    let mut rest = bytes.as_slice();
    while !rest.is_empty() {
        let (used, events) = attempt.push(rest)?;
        if stream {
            delivery.encode_events(&attempt, &events)?;
        }
        if used == 0 {
            return Err("no intake progress".into());
        }
        rest = &rest[used..];
    }
    attempt.finish()?;
    if stream {
        delivery.finish_stream(&attempt)?;
    } else {
        delivery.encode_json(&attempt)?;
    }
    println!("consumed and projected");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        // Only use this diagnostic on authorized synthetic captures. Decoder errors
        // name the failed contract; response bodies and successful output stay private.
        eprintln!("replay rejected: {error}");
        std::process::exit(1);
    }
}
