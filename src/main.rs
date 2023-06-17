#![allow(clippy::identity_op, clippy::too_many_arguments, dead_code)]

use stb_image::stb_image::bindgen::stbi_set_flip_vertically_on_load;

use crate::psx_structs::VertexPSX;
mod collision;
mod helpers;
mod psx_structs;
mod visual;
use clap::Parser;

struct MeshGridEntry {
    triangles: Vec<VertexPSX>,
    quads: Vec<VertexPSX>,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Obj file input path, with the extension .obj
    #[arg(short, long)]
    input: String,

    /// Mesh file output path, without the extension
    #[arg(short, long)]
    output: Option<String>,

    /// Whether this is a collision mesh or not.
    #[arg(short, long)]
    collision: bool,
}

fn main() {
    let args = Cli::parse();
    unsafe {
        stbi_set_flip_vertically_on_load(1);
    }

    let input_obj = args.input.clone();
    let (output_txc, output_msh, output_col) = match args.output {
        None => (
            args.input.replace(".obj", ".txc"),
            args.input.replace(".obj", ".msh"),
            args.input.replace(".obj", ".col"),
        ),
        Some(output) => (
            output.clone() + ".txc",
            output.clone() + ".msh",
            output + ".col",
        ),
    };

    match args.collision {
        false => visual::obj2msh_txc(input_obj, output_msh, output_txc),
        true => collision::obj2col(input_obj, output_col),
    }
}
