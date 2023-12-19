use std::{fs::File, io::Write, path::Path};

use crate::helpers::validate;
#[derive(Clone, Copy)]
pub struct VertexPSX {
    pub pos_x: i16,
    pub pos_y: i16,
    pub pos_z: i16,
    pub color_r: u8,
    pub color_g: u8,
    pub color_b: u8,
    pub tex_u: u8,
    pub tex_v: u8,
    pub texture_id: u8,
}

#[derive(Clone, Copy)]
pub struct CollVertexPSX {
    pub pos_x: i16,
    pub pos_y: i16,
    pub pos_z: i16,
    pub terrain_id: u16,
}

pub struct CollModelPSX {
    pub verts: Vec<CollVertexPSX>,
}
impl CollModelPSX {
    pub fn save(&self, output_col: &Path) {
        // Open output file
        let mut file = File::create(output_col).unwrap();

        // Write file magic
        validate(file.write("FCOL".as_bytes()));

        // Write number of triangles
        validate(file.write((self.verts.len() as u32).to_le_bytes().as_slice()));

        // Write triangle data
        for vert in &self.verts {
            validate(file.write(vert.pos_x.to_le_bytes().as_slice()));
            validate(file.write(vert.pos_y.to_le_bytes().as_slice()));
            validate(file.write(vert.pos_z.to_le_bytes().as_slice()));
            validate(file.write(vert.terrain_id.to_le_bytes().as_slice()));
        }
    }
}

pub struct MeshPSX {
    pub verts: Vec<VertexPSX>,
    pub n_triangles: usize,
    pub n_quads: usize,
    pub name: String,
}

pub struct ModelPSX {
    pub meshes: Vec<MeshPSX>,
}

#[derive(Clone, Copy)]
pub struct MeshDesc {
    pub vertex_start: u16,
    pub n_triangles: u16,
    pub n_quads: u16,
    pub x_min: i16,
    pub x_max: i16,
    pub y_min: i16,
    pub y_max: i16,
    pub z_min: i16,
    pub z_max: i16,
    pub pad: i16,
}

pub struct TextureCollectionPSX {
    pub texture_cells: Vec<TextureCellPSX>,
    pub texture_names: Vec<String>,
}

pub struct TextureCellPSX {
    pub texture_data: Vec<u8>,
    pub palette: Vec<u16>,
    pub texture_width: u8,
    pub texture_height: u8,
    pub avg_color: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TextureCellBinary {
    pub sector_offset_texture: u8,
    pub palette_index: u8,
    pub texture_width: u8,
    pub texture_height: u8,
    pub avg_color: u32,
}

impl VertexPSX {
    pub fn get_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(self.pos_x.to_le_bytes());
        bytes.extend(self.pos_y.to_le_bytes());
        bytes.extend(self.pos_z.to_le_bytes());
        bytes.extend(self.color_r.to_le_bytes());
        bytes.extend(self.color_g.to_le_bytes());
        bytes.extend(self.color_b.to_le_bytes());
        bytes.extend(self.tex_u.to_le_bytes());
        bytes.extend(self.tex_v.to_le_bytes());
        bytes.extend(self.texture_id.to_le_bytes());
        bytes
    }
}

impl CollVertexPSX {
    pub fn get_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(self.pos_x.to_le_bytes());
        bytes.extend(self.pos_y.to_le_bytes());
        bytes.extend(self.pos_z.to_le_bytes());
        bytes.extend(self.terrain_id.to_le_bytes());
        bytes
    }
}

impl ModelPSX {
    pub fn new() -> ModelPSX {
        ModelPSX { meshes: Vec::new() }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<usize> {
        // Create binary array of data
        let mut raw_vertex_data = Vec::<VertexPSX>::new();
        let mut mesh_descs = Vec::<MeshDesc>::new();

        // For each submesh, add the vertices to the array, and store 32-bit offsets to the start of each of them
        for mesh in self.meshes.as_slice() {
            // Find AABB extremes
            let mut x_max = -32768;
            let mut x_min = 32767;
            let mut y_max = -32768;
            let mut y_min = 32767;
            let mut z_max = -32768;
            let mut z_min = 32767;
            for vertex in &mesh.verts {
                x_max = x_max.max(vertex.pos_x);
                x_min = x_min.min(vertex.pos_x);
                y_max = y_max.max(vertex.pos_y);
                y_min = y_min.min(vertex.pos_y);
                z_max = z_max.max(vertex.pos_z);
                z_min = z_min.min(vertex.pos_z);
            }

            mesh_descs.push(MeshDesc {
                vertex_start: raw_vertex_data.len() as u16,
                n_triangles: mesh.n_triangles as u16,
                n_quads: mesh.n_quads as u16,
                x_min,
                x_max,
                y_min,
                y_max,
                z_min,
                z_max,
                pad: 0,
            });
            println!("{} verts ({} polygons)", mesh.verts.len(), mesh.n_quads + mesh.n_triangles);
            for vertex in &mesh.verts {
                raw_vertex_data.push(*vertex);
            }
        }

        // Open output file
        let mut file = File::create(path)?;
        let mut raw_data = Vec::<u8>::new();

        // Mesh descs
        let offset_mesh_desc = raw_data.len();
        for value in mesh_descs {
            raw_data.extend(&value.vertex_start.to_le_bytes());
            raw_data.extend(&value.n_triangles.to_le_bytes());
            raw_data.extend(&value.n_quads.to_le_bytes());
            raw_data.extend(&value.x_min.to_le_bytes());
            raw_data.extend(&value.x_max.to_le_bytes());
            raw_data.extend(&value.y_min.to_le_bytes());
            raw_data.extend(&value.y_max.to_le_bytes());
            raw_data.extend(&value.z_min.to_le_bytes());
            raw_data.extend(&value.z_max.to_le_bytes());
            raw_data.extend(&(0u16).to_le_bytes());
        }

        // Align to word
        while raw_data.len() % 4 != 0 {
            raw_data.push(0);
        }

        // Vertex data
        let offset_vertex_data = raw_data.len();
        for vertex in raw_vertex_data {
            raw_data.extend(&vertex.get_bytes());
        }

        // Align to word
        while raw_data.len() % 4 != 0 {
            raw_data.push(0);
        }

        // Mesh names
        let offset_mesh_names = raw_data.len();
        for mesh in self.meshes.as_slice() {
            raw_data.extend(&(mesh.name.len() as u32).to_le_bytes());
            raw_data.extend(mesh.name.as_bytes());
        }

        // Write everything
        validate(file.write("FMSH".as_bytes())); // file_magic
        validate(file.write(&(self.meshes.len() as u32).to_le_bytes())); //n_submeshes
        validate(file.write(&(offset_mesh_desc as u32).to_le_bytes()));
        validate(file.write(&(offset_vertex_data as u32).to_le_bytes()));
        validate(file.write(&(offset_mesh_names as u32).to_le_bytes()));
        validate(file.write(raw_data.as_slice()));

        Ok(0)
    }
}

impl TextureCollectionPSX {
    pub fn new() -> Self {
        TextureCollectionPSX {
            texture_cells: Vec::new(),
            texture_names: Vec::new(),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<usize> {
        // Open output file
        let mut file = File::create(path)?;

        // Write file magic
        validate(file.write("FTXC".as_bytes()));

        // Write number of texture cells and palettes
        validate(file.write(&(self.texture_cells.len() as u32).to_le_bytes()));

        // Create binary data buffers for each part
        let mut bin_texture_cell_descs: Vec<u8> = Vec::new();
        let mut bin_palettes: Vec<u8> = Vec::new();
        let mut bin_texture_data: Vec<u8> = Vec::new();

        // Populate these buffers
        for i in 0..self.texture_cells.len() {
            let cell = &self.texture_cells[i];
            // Palettes
            {
                let palette = &cell.palette;
                for color in palette {
                    bin_palettes.push(((color >> 0) & 0xFF) as u8);
                    bin_palettes.push(((color >> 8) & 0xFF) as u8);
                }
            }

            // Texture data
            {
                // Add texture to the texture binary
                // It's aligned in such a way that >=64x64 textures are aligned to CD sectors,
                // and anything lower will align to a subdivision of the CD sector
                let grid_align = 2048;

                // Determine the number of bytes to add to align the texture data
                let curr_position = bin_texture_data.len() as u32;
                let n_bytes_to_add =
                    ((curr_position + (grid_align - 1)) & !(grid_align - 1)) - curr_position;

                println!("curr_position: {curr_position}");
                println!("n_bytes_to_add: {n_bytes_to_add}");

                // Pad to this align
                bin_texture_data.resize(bin_texture_data.len() + n_bytes_to_add as usize, 0);

                // Add the texture data to the binary array
                bin_texture_data.extend(&cell.texture_data);

                // Write texture offset
                bin_texture_cell_descs.push(((curr_position + n_bytes_to_add) / 2048) as u8);

                // Write palette index
                bin_texture_cell_descs.extend_from_slice(&(i as u8).to_le_bytes());

                // Write texture dimensions
                bin_texture_cell_descs.push(cell.texture_width);
                bin_texture_cell_descs.push(cell.texture_height);

                // Write texture dimensions
                bin_texture_cell_descs.extend_from_slice(&cell.avg_color.to_le_bytes());
            }
        }

        // I guess we can just write these in any order at this point, since the offsets will be stored in the main header
        let mut cursor: u32 = 0;

        // Write offset to texture cell descs
        validate(file.write(&(cursor).to_le_bytes()));
        cursor += bin_texture_cell_descs.len() as u32;

        // Write offset to palettes
        validate(file.write(&(cursor).to_le_bytes()));
        cursor += bin_palettes.len() as u32;

        // Align the texture data to a CD sector. This allows for some neat optimizations
        let real_cursor = cursor + 24;
        let bytes_to_pad = ((real_cursor + 2047) & !2047) - real_cursor;
        cursor += bytes_to_pad;

        // Write offset to textures
        validate(file.write(&(cursor).to_le_bytes()));
        //cursor += bin_texture_data.len() as u32;

        // todo: name table
        validate(file.write(&(0u32).to_le_bytes()));

        // Write the raw buffers now, in the right order
        validate(file.write(bin_texture_cell_descs.as_slice()));
        validate(file.write(bin_palettes.as_slice()));

        // Pad with zeroes
        for _ in 0..bytes_to_pad {
            validate(file.write(&[0u8]));
        }

        validate(file.write(bin_texture_data.as_slice()));

        Ok(0)
    }
}
