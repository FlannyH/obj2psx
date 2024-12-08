#![allow(clippy::identity_op, clippy::too_many_arguments, dead_code)]

use std::path::Path;

use stb_image::stb_image::bindgen::stbi_set_flip_vertically_on_load;

use crate::psx_structs::VertexPSX;
mod bsp;
mod collision;
mod helpers;
mod kmeans;
mod psx_structs;
mod renderer;
mod texture_page;
mod visual;
use clap::Parser;

pub struct MeshGridEntry {
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

    /// Whether this mesh should be split into regions
    #[arg(short, long)]
    split: bool,

    /// Whether we want to print debug messages or not
    #[arg(short, long)]
    verbose: bool
}

fn main() {
    let args = Cli::parse();
    unsafe {
        stbi_set_flip_vertically_on_load(0);
    }

    if args.verbose {
        env_logger::Builder::new().filter_level(log::LevelFilter::Debug).init();
    } else {
        env_logger::Builder::new().filter_level(log::LevelFilter::Info).init();
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
            false => visual::obj2msh_txc(input, output_msh, output_txc, args.page, args.split),
            true => collision::obj2col(input, output_col),
        }
        return;
    }
    if input.ends_with(".png") {
        let output_txc = match args.output {
            None => args.input.replace(".png", ".txc"),
            Some(output) => output,
        };
        texture_page::txc_from_page(Path::new(&input))
            .save(Path::new(&output_txc))
            .unwrap();
    }
}
