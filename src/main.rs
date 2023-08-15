#![allow(clippy::identity_op, clippy::too_many_arguments, dead_code)]

use std::path::Path;

use stb_image::stb_image::bindgen::stbi_set_flip_vertically_on_load;

use crate::psx_structs::VertexPSX;
mod collision;
mod helpers;
mod psx_structs;
mod visual;
mod texture_page;
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

    /// Whether this uses 8-bit 256x256 texture pages or 4-bit 64x64 texture cells
    #[arg(short, long)]
    page: bool,
}

fn main() {
    let args = Cli::parse();
    unsafe {
        stbi_set_flip_vertically_on_load(0);
    }

    let input = args.input.clone();
    if input.ends_with(".obj") {
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
            false => visual::obj2msh_txc(input, output_msh, output_txc, args.page),
            true => collision::obj2col(input, output_col),
        }
        return;
    }   
    if input.ends_with(".png") {
        let output_txc = match args.output {
            None => args.input.replace(".png", ".txc"),
            Some(output) => output,
        };
        texture_page::txc_from_page(Path::new(&input)).save(Path::new(&output_txc)).unwrap();
    }
}
