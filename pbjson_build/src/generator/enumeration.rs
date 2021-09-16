//! This module contains the code to generate Serialize and Deserialize
//! implementations for enumeration type
//!
//! An enumeration should be decode-able from the full string variant name
//! or its integer tag number, and should encode to the string representation

use super::{
    write_deserialize_end, write_deserialize_start, write_serialize_end, write_serialize_start,
    Config, Indent,
};
use crate::descriptor::{EnumDescriptor, TypePath};
use crate::generator::write_fields_array;
use std::io::{Result, Write};

pub fn generate_enum<W: Write>(
    config: &Config,
    path: &TypePath,
    descriptor: &EnumDescriptor,
    writer: &mut W,
) -> Result<()> {
    let rust_type = config.rust_type(path);

    let variants: Vec<_> = descriptor
        .values
        .iter()
        .map(|variant| {
            let variant_name = variant.name.clone().unwrap();
            let rust_variant = config.rust_variant(path, &variant_name);
            (variant_name, rust_variant)
        })
        .collect();

    // Generate Serialize
    write_serialize_start(0, &rust_type, writer)?;
    writeln!(writer, "{}let variant = match self {{", Indent(2))?;
    for (variant_name, rust_variant) in &variants {
        writeln!(
            writer,
            "{}Self::{} => \"{}\",",
            Indent(3),
            rust_variant,
            variant_name
        )?;
    }
    writeln!(writer, "{}}};", Indent(2))?;

    writeln!(writer, "{}serializer.serialize_str(variant)", Indent(2))?;
    write_serialize_end(0, writer)?;

    // Generate Deserialize
    write_deserialize_start(0, &rust_type, writer)?;
    write_fields_array(writer, 2, variants.iter().map(|(name, _)| name.as_str()))?;
    write_visitor(writer, 2, &rust_type, &variants)?;

    // Use deserialize_any to allow users to provide integers or strings
    writeln!(
        writer,
        "{}deserializer.deserialize_any(GeneratedVisitor)",
        Indent(2)
    )?;

    write_deserialize_end(0, writer)?;
    Ok(())
}

fn write_visitor<W: Write>(
    writer: &mut W,
    indent: usize,
    rust_type: &str,
    variants: &[(String, String)],
) -> Result<()> {
    // Protobuf supports deserialization of enumerations both from string and integer values
    writeln!(
        writer,
        r#"{indent}struct GeneratedVisitor;

{indent}impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {{
{indent}    type Value = {rust_type};

{indent}    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
{indent}        write!(formatter, "expected one of: {{:?}}", &FIELDS)
{indent}    }}

{indent}    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
{indent}    where
{indent}        E: serde::de::Error,
{indent}    {{
{indent}        use std::convert::TryFrom;
{indent}        i32::try_from(v)
{indent}            .ok()
{indent}            .and_then({rust_type}::from_i32)
{indent}            .ok_or_else(|| {{
{indent}                serde::de::Error::invalid_value(serde::de::Unexpected::Signed(v), &self)
{indent}            }})
{indent}    }}

{indent}    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
{indent}    where
{indent}        E: serde::de::Error,
{indent}    {{
{indent}        use std::convert::TryFrom;
{indent}        i32::try_from(v)
{indent}            .ok()
{indent}            .and_then({rust_type}::from_i32)
{indent}            .ok_or_else(|| {{
{indent}                serde::de::Error::invalid_value(serde::de::Unexpected::Unsigned(v), &self)
{indent}            }})
{indent}    }}

{indent}    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
{indent}    where
{indent}        E: serde::de::Error,
{indent}    {{"#,
        indent = Indent(indent),
        rust_type = rust_type,
    )?;

    writeln!(writer, "{}match value {{", Indent(indent + 2))?;
    for (variant_name, rust_variant) in variants {
        writeln!(
            writer,
            "{}\"{}\" => Ok({}::{}),",
            Indent(indent + 3),
            variant_name,
            rust_type,
            rust_variant
        )?;
    }

    writeln!(
        writer,
        "{indent}_ => Err(serde::de::Error::unknown_variant(value, FIELDS)),",
        indent = Indent(indent + 3)
    )?;
    writeln!(writer, "{}}}", Indent(indent + 2))?;
    writeln!(writer, "{}}}", Indent(indent + 1))?;
    writeln!(writer, "{}}}", Indent(indent))
}
