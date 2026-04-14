use clap::Parser;
use epics_pva_rs::client::PvaClient;
use epics_pva_rs::{FieldType, StructureDesc};

#[derive(Parser)]
#[command(name = "rpvainfo", about = "Show EPICS PV type info via pvAccess")]
struct Args {
    /// PV names to query
    #[arg(required = true)]
    pv_names: Vec<String>,
}

fn format_field_type(ft: &FieldType) -> String {
    match ft {
        FieldType::Scalar(tc) => format!("{tc:?}"),
        FieldType::ScalarArray(tc) => format!("{tc:?}[]"),
        FieldType::String => "string".to_string(),
        FieldType::BoundedString(n) => format!("string({n})"),
        FieldType::Structure(desc) => format_structure(desc),
        FieldType::StructureArray(desc) => format!("{}[]", format_structure(desc)),
        FieldType::Union(fields) => {
            let mut out = String::from("union\n");
            for field in fields {
                out.push_str(&format!(
                    "    {} {}\n",
                    format_field_type(&field.field_type),
                    field.name
                ));
            }
            out
        }
        _ => format!("{ft:?}"),
    }
}

fn format_structure(desc: &StructureDesc) -> String {
    format_structure_indent(desc, 0)
}

fn format_structure_indent(desc: &StructureDesc, indent: usize) -> String {
    let mut out = String::new();
    let id = desc.struct_id.as_deref().unwrap_or("structure");
    out.push_str(id);
    out.push('\n');
    for field in &desc.fields {
        let prefix = "    ".repeat(indent + 1);
        match &field.field_type {
            FieldType::Structure(sub) => {
                out.push_str(&format!(
                    "{prefix}{} {}\n",
                    field.name,
                    format_structure_indent(sub, indent + 1)
                ));
            }
            other => {
                out.push_str(&format!(
                    "{prefix}{} {}\n",
                    format_field_type(other),
                    field.name,
                ));
            }
        }
    }
    out
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let client = PvaClient::new().expect("failed to create PVA client");

    let mut failed = false;
    for pv_name in &args.pv_names {
        match client.pvainfo(pv_name).await {
            Ok(desc) => {
                println!("{pv_name}:");
                print!("{}", format_structure(&desc));
            }
            Err(e) => {
                eprintln!("{pv_name}: {e}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
