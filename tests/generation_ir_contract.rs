//! Characterizes the provider-neutral static Generation IR before production wiring exists.

use openbridge::ir::generation::{
    project_semantic_requirements, ContentPart, GenerationRequest, InputItem, Instruction,
    InstructionAuthority, Message, MessageRole, TextValue,
};

#[test]
fn ordered_input_projects_instruction_and_text_requirements() {
    let instruction = Instruction::new(
        InstructionAuthority::System,
        TextValue::new("Answer concisely", 1_024).expect("instruction must fit"),
    );
    let message = Message::new(
        MessageRole::User,
        vec![ContentPart::Text(
            TextValue::new("hello", 1_024).expect("message text must fit"),
        )],
    )
    .expect("message must contain content");

    let request = GenerationRequest::new(vec![
        InputItem::Instruction(instruction),
        InputItem::Message(message),
    ])
    .expect("ordered request must be valid");

    assert!(matches!(request.input()[0], InputItem::Instruction(_)));
    assert!(matches!(request.input()[1], InputItem::Message(_)));

    let requirements = project_semantic_requirements(&request);
    assert!(requirements.instructions());
    assert_eq!(requirements.text_parts(), 1);
    assert!(!requirements.function_tools());
    assert!(!requirements.image_input());
    assert!(!requirements.audio_input());
    assert!(!requirements.file_input());
}
