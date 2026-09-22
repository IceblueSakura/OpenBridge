use crate::semantic::value::Text;
#[derive(Clone,Copy,Debug,Eq,PartialEq)] pub enum ResourceKind{Image,Audio,File}
#[derive(Clone,Debug,Eq,PartialEq)] pub enum ResourceLocation{Url(Text),Inline{media_type:Text,data_base64:Text},OpaqueReference(Text)}
#[derive(Clone,Debug,Eq,PartialEq)] pub struct Resource{pub kind:ResourceKind,pub location:ResourceLocation}
