use std::{collections::HashMap, path::Path};

use exoquant::{
    generate_palette,
    optimizer::{self, Optimizer},
    Color, SimpleColorSpace,
};
use log::{debug, warn};
use tobj::LoadOptions;

use crate::{
    bsp::split_bsp,
    kmeans::kmeans_cluster,
    psx_structs::{MeshPSX, ModelPSX, TextureCellPSX, TextureCollectionPSX, VertexPSX},
    MeshGridEntry,
};

pub fn obj2msh_txc(
    input_obj: String,
    output_msh: String,
    output_txc: String,
    using_texture_page: bool,
    split: bool,
) {
    let (models, materials) = tobj::load_obj(
        &input_obj,
        &LoadOptions {
            single_index: true,
            ..Default::default()
        },
    )
    .expect("Failed to OBJ load file");

    // Create a material mapping to filter out special material types like occluders
    let mut material_mapping = vec![];
    let mut psx_id_tex_mapping = vec![];
    if let Ok(materials_vec) = &materials {
        for material in materials_vec {
            // First let's figure out what we have in this material. Is it textured? Is it untextured? Is it an occluder?
            let psx_tex_id;

            if material.name.contains("occlude") {
                psx_tex_id = 254;
            } else if let Some(tex_path) = &material.diffuse_texture {
                // If the texture path is already in here, reuse the corresponding material index
                if let Some(already_added_id) =
                    psx_id_tex_mapping.iter().position(|x| *x == *tex_path)
                {
                    psx_tex_id = already_added_id;
                }
                // Otherwise add it to the list
                else {
                    psx_tex_id = psx_id_tex_mapping.len();
                    psx_id_tex_mapping.push(tex_path.to_string());
                }
            } else {
                psx_tex_id = 255;
            }

            // Now we know what this is, let's add it to the mapping
            material_mapping.push(psx_tex_id);
        }
    }

    // debug
    for (obj_mat_id, psx_tex_id) in material_mapping.iter().enumerate() {
        let tex_name = match *psx_tex_id {
            255 => "(no texture)".to_string(),
            254 => "(occluder)".to_string(),
            x => psx_id_tex_mapping.get(x).unwrap().to_string(),
        };
        debug!("{obj_mat_id}: tex id {psx_tex_id}: {}", tex_name);
    }

    let mut model_psx = ModelPSX::new();
    let mut txc_psx = TextureCollectionPSX::new();
    let mut mesh_map: HashMap<String, MeshGridEntry> = HashMap::new();

    // Loop over every mesh in the model. We want to combine them all.
    for model in &models {
        let mut curr_index = 0;
        let mut triangles;
        let mut quads;
        if mesh_map.contains_key(&model.name) {
            triangles = mesh_map.get(&model.name).unwrap().triangles.clone();
            quads = mesh_map.get(&model.name).unwrap().quads.clone();
        } else {
            triangles = Vec::new();
            quads = Vec::new();
        }

        let face_arities = match model.mesh.face_arities.is_empty() {
            false => model.mesh.face_arities.clone(),
            true => std::iter::repeat(3)
                .take(model.mesh.indices.len() / 3)
                .collect(),
        };

        for arity in &face_arities {
            let mut curr_primitive = Vec::<VertexPSX>::new();
            for in_face_index in curr_index as usize..(curr_index + arity) as usize {
                let index = model.mesh.indices[in_face_index] as usize;
                let (h, mut s, l) = rgb_to_hsl((
                    model.mesh.vertex_color[index * 3 + 0],
                    model.mesh.vertex_color[index * 3 + 1],
                    model.mesh.vertex_color[index * 3 + 2],
                ));
                //l = l.powf(1.0/1.0);
                s *= 1.25;
                let (r, g, b) = hsl_to_rgb((h, s, l));
                let vert = VertexPSX {
                    pos_x: (model.mesh.positions[index * 3 + 0] * -1024.0).clamp(-32768.0, 32767.0)
                        as i16,
                    pos_y: (model.mesh.positions[index * 3 + 1] * -1024.0).clamp(-32768.0, 32767.0)
                        as i16,
                    pos_z: (model.mesh.positions[index * 3 + 2] * 1024.0).clamp(-32768.0, 32767.0)
                        as i16,
                    color_r: (r * 255.0).clamp(0.0, 255.0) as u8,
                    color_g: (g * 255.0).clamp(0.0, 255.0) as u8,
                    color_b: (b * 255.0).clamp(0.0, 255.0) as u8,
                    tex_u: (model.mesh.texcoords[index * 2 + 0] * 255.0).round() as u8,
                    tex_v: (255.0 - (model.mesh.texcoords[index * 2 + 1] * 255.0)).round() as u8,
                    texture_id: match model.mesh.material_id {
                        None => 255,
                        Some(a) => *(material_mapping.get(a).unwrap_or(&255)) as u8,
                    },
                    normal_x: (model.mesh.normals[index * 3 + 0] * 127.0).clamp(-127.0, 127.0)
                        as i8,
                    normal_y: (model.mesh.normals[index * 3 + 1] * 127.0).clamp(-127.0, 127.0)
                        as i8,
                    normal_z: (model.mesh.normals[index * 3 + 2] * 127.0).clamp(-127.0, 127.0)
                        as i8,
                };
                curr_primitive.push(vert);
            }
            match arity {
                3 => {
                    // Find size of triangle's bounding box and store it in the second primitive's tex_id field
                    let mut x_min = i32::MAX;
                    let mut y_min = i32::MAX;
                    let mut z_min = i32::MAX;
                    let mut x_max = i32::MIN;
                    let mut y_max = i32::MIN;
                    let mut z_max = i32::MIN;
                    for i in [0, 2, 1] {
                        x_min = x_min.min(curr_primitive[i].pos_x as i32);
                        y_min = y_min.min(curr_primitive[i].pos_y as i32);
                        z_min = z_min.min(curr_primitive[i].pos_z as i32);
                        x_max = x_max.max(curr_primitive[i].pos_x as i32);
                        y_max = y_max.max(curr_primitive[i].pos_y as i32);
                        z_max = z_max.max(curr_primitive[i].pos_z as i32);
                    }
                    x_max -= x_min;
                    y_max -= y_min;
                    z_max -= z_min;
                    let size = ((x_max*x_max + y_max*y_max + z_max*z_max) as f64).sqrt();
                    curr_primitive[2].texture_id = (size / 16.0).clamp(0.0, 255.0) as u8; // bigger number means bigger primitive

                    for i in [0, 2, 1] {
                        triangles.push(curr_primitive[i]);
                    }
                },
                4 => {
                    // Find size of quad's bounding box and store it in the second primitive's tex_id field
                    let mut x_min = i32::MAX;
                    let mut y_min = i32::MAX;
                    let mut z_min = i32::MAX;
                    let mut x_max = i32::MIN;
                    let mut y_max = i32::MIN;
                    let mut z_max = i32::MIN;
                    for i in [0, 3, 1, 2] {
                        x_min = x_min.min(curr_primitive[i].pos_x as i32);
                        y_min = y_min.min(curr_primitive[i].pos_y as i32);
                        z_min = z_min.min(curr_primitive[i].pos_z as i32);
                        x_max = x_max.max(curr_primitive[i].pos_x as i32);
                        y_max = y_max.max(curr_primitive[i].pos_y as i32);
                        z_max = z_max.max(curr_primitive[i].pos_z as i32);
                    }
                    x_max -= x_min;
                    y_max -= y_min;
                    z_max -= z_min;
                    let size = ((x_max*x_max + y_max*y_max + z_max*z_max) as f64).sqrt();
                    curr_primitive[3].texture_id = (size / 16.0).clamp(0.0, 255.0) as u8; // bigger number means bigger primitive

                    for i in [0, 3, 1, 2]{
                        quads.push(curr_primitive[i]);
                    }
                },
                _ => warn!("found polygon with more than 4 vertices! make sure the mesh only contains triangles and quads."),
            };

            curr_index += arity;
        }

        mesh_map.insert(model.name.clone(), MeshGridEntry { triangles, quads });
    }

    let mode = match split {
        true => 4,
        false => 0,
    };

    // Recombine separated meshes based on mesh name
    if mode == 0 {
        // Sort in alphabet order
        let mut sorted_keys: Vec<&String> = mesh_map.keys().collect();
        sorted_keys.sort();

        for key in sorted_keys {
            let value = &mesh_map[key];
            let n_triangles = value.triangles.len() / 3;
            let n_quads = value.quads.len() / 4;
            let mut combined_vector = Vec::<VertexPSX>::new();
            combined_vector.extend(&value.triangles);
            combined_vector.extend(&value.quads);
            model_psx.meshes.push(MeshPSX {
                verts: combined_vector,
                n_triangles,
                n_quads,
                name: key.clone(),
            });
        }
    }
    // 3D grid
    else if mode == 1 {
        let mut grid_map = HashMap::<(i16, i16, i16), MeshGridEntry>::new();
        for value in mesh_map.values() {
            let grid_size = (1800.0, 8000.0, 1800.0);
            for triangle in value.triangles.chunks(3) {
                // Find which gridcell this triangle belongs to
                let avg_pos_x = (triangle[0].pos_x as f64
                    + triangle[1].pos_x as f64
                    + triangle[2].pos_x as f64)
                    / 3.0;
                let avg_pos_y = (triangle[0].pos_y as f64
                    + triangle[1].pos_y as f64
                    + triangle[2].pos_y as f64)
                    / 3.0;
                let avg_pos_z = (triangle[0].pos_z as f64
                    + triangle[1].pos_z as f64
                    + triangle[2].pos_z as f64)
                    / 3.0;
                let grid_x = (avg_pos_x / grid_size.0) as i16;
                let grid_y = (avg_pos_y / grid_size.1) as i16;
                let grid_z = (avg_pos_z / grid_size.2) as i16;
                let mesh_psx =
                    grid_map
                        .entry((grid_x, grid_y, grid_z))
                        .or_insert_with(|| MeshGridEntry {
                            triangles: Vec::new(),
                            quads: Vec::new(),
                        });
                mesh_psx.triangles.extend(triangle);
            }
            for quad in value.quads.chunks(4) {
                // Find which gridcell this triangle belongs to
                let avg_pos_x = (quad[0].pos_x as f64
                    + quad[1].pos_x as f64
                    + quad[2].pos_x as f64
                    + quad[3].pos_x as f64)
                    / 3.0;
                let avg_pos_y = (quad[0].pos_y as f64
                    + quad[1].pos_y as f64
                    + quad[2].pos_y as f64
                    + quad[3].pos_y as f64)
                    / 3.0;
                let avg_pos_z = (quad[0].pos_z as f64
                    + quad[1].pos_z as f64
                    + quad[2].pos_z as f64
                    + quad[3].pos_z as f64)
                    / 3.0;
                let grid_x = (avg_pos_x / grid_size.0) as i16;
                let grid_y = (avg_pos_y / grid_size.1) as i16;
                let grid_z = (avg_pos_z / grid_size.2) as i16;
                let mesh_psx =
                    grid_map
                        .entry((grid_x, grid_y, grid_z))
                        .or_insert_with(|| MeshGridEntry {
                            triangles: Vec::new(),
                            quads: Vec::new(),
                        });
                mesh_psx.quads.extend(quad);
            }
        }
        for (_key, mesh) in grid_map {
            let n_triangles = mesh.triangles.len() / 3;
            let n_quads = mesh.quads.len() / 4;
            let mut combined_vector = Vec::<VertexPSX>::new();
            combined_vector.extend(mesh.triangles);
            combined_vector.extend(mesh.quads);
            model_psx.meshes.push(MeshPSX {
                verts: combined_vector,
                n_triangles,
                n_quads,
                name: "(null)".to_string(),
            });
        }
    }
    // BSP
    else if mode == 2 {
        let mesh_entries = split_bsp(mesh_map, 250);
        for mesh in mesh_entries {
            let n_triangles = mesh.triangles.len() / 3;
            let n_quads = mesh.quads.len() / 4;
            let mut combined_vector = Vec::<VertexPSX>::new();
            combined_vector.extend(mesh.triangles);
            combined_vector.extend(mesh.quads);
            model_psx.meshes.push(MeshPSX {
                verts: combined_vector,
                n_triangles,
                n_quads,
                name: "(null)".to_string(),
            });
        }
    }
    // K-means clustering
    else if mode == 3 {
        let mesh_entries = kmeans_cluster(mesh_map, 50);
        for mesh in mesh_entries {
            let n_triangles = mesh.triangles.len() / 3;
            let n_quads = mesh.quads.len() / 4;
            let mut combined_vector = Vec::<VertexPSX>::new();
            combined_vector.extend(mesh.triangles);
            combined_vector.extend(mesh.quads);
            model_psx.meshes.push(MeshPSX {
                verts: combined_vector,
                n_triangles,
                n_quads,
                name: "(null)".to_string(),
            });
        }
    }
    // Equally split on mesh bounding box
    else if mode == 4 {
        let target_polygon_count_per_mesh = 80;

        // Loop over each mesh
        for (name, mesh) in mesh_map {
            // Don't split if the number of polygons is low anyway
            if mesh.triangles.len() + mesh.quads.len() < target_polygon_count_per_mesh {
                // Create a mesh
                let mut combined_verts = Vec::new();
                combined_verts.extend(mesh.triangles.iter());
                combined_verts.extend(mesh.quads.iter());
                model_psx.meshes.push(MeshPSX {
                    verts: combined_verts,
                    n_triangles: mesh.triangles.len() / 3,
                    n_quads: mesh.quads.len() / 4,
                    name,
                });
                continue;
            }

            // Get bounding box
            let (mut min_x, mut min_y, mut min_z) = (i16::MAX, i16::MAX, i16::MAX);
            let (mut max_x, mut max_y, mut max_z) = (i16::MIN, i16::MIN, i16::MIN);
            for vertex in &mesh.triangles {
                min_x = min_x.min(vertex.pos_x);
                min_y = min_y.min(vertex.pos_y);
                min_z = min_z.min(vertex.pos_z);
                max_x = max_x.max(vertex.pos_x);
                max_y = max_y.max(vertex.pos_y);
                max_z = max_z.max(vertex.pos_z);
            }

            for vertex in &mesh.quads {
                min_x = min_x.min(vertex.pos_x);
                min_y = min_y.min(vertex.pos_y);
                min_z = min_z.min(vertex.pos_z);
                max_x = max_x.max(vertex.pos_x);
                max_y = max_y.max(vertex.pos_y);
                max_z = max_z.max(vertex.pos_z);
            }

            // Calculate size
            let (size_x, size_y, size_z) = (max_x - min_x, max_y - min_y, max_z - min_z);

            // Figure out the best split count - a bit expensive, since we brute force it, but that's fine for lower poly meshes like on PS1
            let mut current_best: Vec<MeshPSX> = Vec::new();
            let mut current_best_error = i64::MAX;
            for splits_x in 1..6 {
                for splits_y in 1..6 {
                    for splits_z in 1..6 {
                        let mut meshes_to_add: Vec<MeshPSX> = Vec::new();
                        split_equal_based_on_aabb(
                            &name,
                            splits_z,
                            min_z,
                            size_z,
                            splits_y,
                            min_y,
                            size_y,
                            splits_x,
                            min_x,
                            size_x,
                            &mesh,
                            &mut meshes_to_add,
                        );

                        // Calculate score. How big is the deviation from the target we want?
                        let mut error = 0;
                        for mesh in &meshes_to_add {
                            error += (mesh.n_quads as i64 + mesh.n_triangles as i64
                                - target_polygon_count_per_mesh as i64)
                                .abs();
                            error += (splits_x - splits_y).abs() as i64 * 20
                                / (splits_x + splits_y + splits_z) as i64;
                            error += (splits_y - splits_z).abs() as i64 * 20
                                / (splits_x + splits_y + splits_z) as i64;
                            error += (splits_z - splits_x).abs() as i64 * 20
                                / (splits_x + splits_y + splits_z) as i64;
                        }

                        // Is it better than before? Store the result
                        if error < current_best_error && !meshes_to_add.is_empty() {
                            current_best.clear();
                            current_best.extend(meshes_to_add);
                            current_best_error = error;
                        }
                    }
                }
            }

            model_psx.meshes.extend(current_best);
        }
    }

    for tex_path in psx_id_tex_mapping {
        let mut tex_data_src = vec![0xFF; 64 * 64 * 4];
        let mut depth = 4;
        let mut width = 64;
        let mut height = 64;
        let name;

        {
            // Load the image file corresponding to the material
            let input_path = Path::new(&input_obj);
            let parent_directory = input_path.parent().expect("Invalid file path");
            let combined_path = parent_directory.join(tex_path);
            name = String::from(combined_path.to_str().unwrap());
            let raw_image = match stb_image::image::load(&name) {
                stb_image::image::LoadResult::ImageU8(image) => Some(image),
                _ => None,
            };

            // todo: Fit it to 64x64 pixels
            if let Some(raw_image) = raw_image {
                tex_data_src = raw_image.data;
                depth = raw_image.depth;
                width = raw_image.width;
                height = raw_image.height;
            }
        }

        // Create texture cell object
        let mut tex_cell = TextureCellPSX {
            texture_data: Vec::new(),
            palette: Vec::new(),
            texture_width: 64,
            texture_height: 64,
            avg_color: 0,
        };
        // Calculate average color
        let mut avg_r = 0;
        let mut avg_g = 0;
        let mut avg_b = 0;
        let mut avg_a = 0;
        for pixel in tex_data_src.chunks(depth) {
            avg_r += pixel[0] as u32;
            avg_g += pixel[1] as u32;
            avg_b += pixel[2] as u32;
            match depth {
                4 => avg_a += pixel[3] as u32,
                3 => avg_a += 255,
                _ => panic!(),
            }
            if depth == 4 {
                avg_a += pixel[3] as u32;
            } else {
                avg_a += 255
            }
        }
        avg_r /= 64 * 64;
        avg_g /= 64 * 64;
        avg_b /= 64 * 64;
        avg_a /= 64 * 64;
        tex_cell.avg_color = avg_r | avg_b << 8 | avg_g << 16 | avg_a << 24;

        // Quantize it to 16 colours
        let mut tex_data_exoquant = Vec::new();
        let mut has_transparent_pixels = false;
        for pixel in tex_data_src.chunks(depth) {
            match depth {
                4 => {
                    if pixel[3] == 0 {
                        has_transparent_pixels = true;
                        tex_data_exoquant.push(exoquant::Color::new(0, 0, 0, 0))
                    } else {
                        tex_data_exoquant
                            .push(exoquant::Color::new(pixel[0], pixel[1], pixel[2], 255))
                    }
                }
                3 => {
                    tex_data_exoquant.push(exoquant::Color::new(pixel[0], pixel[1], pixel[2], 255))
                }
                _ => panic!(),
            }
        }

        // Make half the histogram transparent pixels so that the quantizer actually generates a palette that contains one of those
        let mut histogram_data = tex_data_exoquant.clone();
        if has_transparent_pixels {
            let n = tex_data_exoquant.len();
            for _ in 0..n {
                histogram_data.push(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                });
            }
        }
        let histogram: &exoquant::Histogram = &histogram_data.iter().cloned().collect();
        let palette = generate_palette(
            histogram,
            &SimpleColorSpace::default(),
            &optimizer::WeightedKMeans,
            match using_texture_page {
                false => 16,
                true => 256,
            },
        );
        let mut palette = optimizer::WeightedKMeans.optimize_palette(
            &SimpleColorSpace::default(),
            &palette,
            histogram,
            8,
        );
        let mut indexed_data = exoquant::Remapper::new(
            &palette,
            &SimpleColorSpace::default(),
            &exoquant::ditherer::Ordered,
        )
        .remap(&tex_data_exoquant, width);

        // If the palette has a transparent pixel color, move it to index 0
        // This makes it easiest to implement using Nintendo DS OpenGL implementation
        // Additionally, make the color full black. This way the PS1 knows it's transparent too
        let mut n_transparent_colors = 0;
        let mut transparent_index = 0;
        for (index, color) in palette.iter_mut().enumerate() {
            if color.a == 0 {
                transparent_index = index;
                n_transparent_colors += 1;
                color.r = 0;
                color.g = 0;
                color.b = 0;
            }
        }

        if n_transparent_colors > 0 {
            // Swap the transparent and first colors
            let temp = palette[0];
            palette[0] = palette[transparent_index];
            palette[transparent_index] = temp;

            // Then remap the indices in the texture
            for pixel in &mut indexed_data {
                if *pixel == transparent_index as u8 {
                    *pixel = 0;
                } else if *pixel == 0 {
                    *pixel = transparent_index as u8;
                }
            }
        } else if n_transparent_colors > 1 {
            warn!("multiple transparent colors detected in texture {name}")
        }

        let color_b = Color {
            r: (avg_r) as u8,
            g: (avg_g) as u8,
            b: (avg_b) as u8,
            a: (avg_a) as u8,
        };
        for fade_level in 0..16 {
            for color in &palette {
                let mut color16: u16 = (color.a as u16).clamp(0, 1) << 15
                    | ((((fade_level * color_b.b as u16) + ((15 - fade_level) * color.b as u16))
                        / 15)
                        >> 3)
                        .clamp(0, 31)
                        << 10
                    | ((((fade_level * color_b.g as u16) + ((15 - fade_level) * color.g as u16))
                        / 15)
                        >> 3)
                        .clamp(0, 31)
                        << 5
                    | ((((fade_level * color_b.r as u16) + ((15 - fade_level) * color.r as u16))
                        / 15)
                        >> 3)
                        .clamp(0, 31)
                        << 0;
                if color.a == 0 {
                    color16 = 0;
                }
                tex_cell.palette.push(color16);
            }
        }

        // Convert indices to 4 bit
        if using_texture_page {
            for i in 0..(width * height) {
                if i < indexed_data.len() {
                    tex_cell.texture_data.push(indexed_data[i]);
                } else {
                    tex_cell.texture_data.push(0);
                    tex_cell.texture_data.push(0);
                    tex_cell.texture_data.push(0);
                    tex_cell.texture_data.push(0);
                }
            }
        } else {
            for i in (0..(width * height)).step_by(2) {
                if (i + 1) < indexed_data.len() {
                    tex_cell
                        .texture_data
                        .push((indexed_data[i + 1] << 4) | (indexed_data[i + 0]));
                } else {
                    tex_cell.texture_data.push(0);
                    tex_cell.texture_data.push(0);
                    tex_cell.texture_data.push(0);
                    tex_cell.texture_data.push(0);
                }
            }
        }

        // Add this cell to the collection
        txc_psx.texture_cells.push(tex_cell);
        txc_psx.texture_names.push(name);
    }

    model_psx.save(Path::new(&output_msh)).unwrap();
    txc_psx.save(Path::new(&output_txc)).unwrap();
}

fn split_equal_based_on_aabb(
    name: &String,
    splits_z: i16,
    min_z: i16,
    size_z: i16,
    splits_y: i16,
    min_y: i16,
    size_y: i16,
    splits_x: i16,
    min_x: i16,
    size_x: i16,
    mesh: &MeshGridEntry,
    meshes_to_add: &mut Vec<MeshPSX>,
) {
    let mut coords_x = vec![i16::MIN];
    let mut coords_y = vec![i16::MIN];
    let mut coords_z = vec![i16::MIN];
    for x in 0..(splits_x + 1) {
        coords_x.push(min_x + ((size_x as f64 / splits_x as f64) * x as f64).ceil() as i16);
    }
    for y in 0..(splits_y + 1) {
        coords_y.push(min_y + ((size_y as f64 / splits_y as f64) * y as f64).ceil() as i16);
    }
    for z in 0..(splits_z + 1) {
        coords_z.push(min_z + ((size_z as f64 / splits_z as f64) * z as f64).ceil() as i16);
    }
    coords_x[1] -= 1;
    coords_y[1] -= 1;
    coords_z[1] -= 1;
    coords_x.push(i16::MAX);
    coords_y.push(i16::MAX);
    coords_z.push(i16::MAX);

    for z in 0..(coords_z.len() - 1) {
        let (curr_min_z, curr_max_z) = (coords_z[z], coords_z[z + 1]);
        for y in 0..(coords_y.len() - 1) {
            let (curr_min_y, curr_max_y) = (coords_y[y], coords_y[y + 1]);
            for x in 0..(coords_x.len() - 1) {
                let (curr_min_x, curr_max_x) = (coords_x[x], coords_x[x + 1]);
                // Find all triangles where the center is inside that bounding box
                let mut tris = Vec::<VertexPSX>::new();
                let mut quads = Vec::<VertexPSX>::new();

                // Select polygons that are inside the bounding box
                {
                    for triangle in mesh.triangles.chunks(3) {
                        // Calculate center
                        let (center_x, center_y, center_z) = (
                            ((triangle[0].pos_x as i32
                                + triangle[1].pos_x as i32
                                + triangle[2].pos_x as i32)
                                / 3) as i16,
                            ((triangle[0].pos_y as i32
                                + triangle[1].pos_y as i32
                                + triangle[2].pos_y as i32)
                                / 3) as i16,
                            ((triangle[0].pos_z as i32
                                + triangle[1].pos_z as i32
                                + triangle[2].pos_z as i32)
                                / 3) as i16,
                        );

                        // If it's in the bounding box
                        if center_x > curr_min_x
                            && center_x <= curr_max_x
                            && center_y > curr_min_y
                            && center_y <= curr_max_y
                            && center_z > curr_min_z
                            && center_z <= curr_max_z
                        {
                            if curr_min_x == i16::MIN || curr_max_x == i16::MAX {
                                warn!("out of bounds triangle? [{center_x}, {center_y}, {center_z}] not in [{min_x}, {min_y}, {min_z}] -> [{}, {}, {}]", min_x + size_x, min_y + size_y, min_z + size_z)
                            }
                            tris.extend_from_slice(triangle);
                        }
                    }

                    for quad in mesh.quads.chunks(4) {
                        // Calculate center
                        let (center_x, center_y, center_z) = (
                            ((quad[0].pos_x as i32
                                + quad[1].pos_x as i32
                                + quad[2].pos_x as i32
                                + quad[3].pos_x as i32)
                                / 4) as i16,
                            ((quad[0].pos_y as i32
                                + quad[1].pos_y as i32
                                + quad[2].pos_y as i32
                                + quad[3].pos_y as i32)
                                / 4) as i16,
                            ((quad[0].pos_z as i32
                                + quad[1].pos_z as i32
                                + quad[2].pos_z as i32
                                + quad[3].pos_z as i32)
                                / 4) as i16,
                        );

                        // If it's in the bounding box
                        if center_x > curr_min_x
                            && center_x <= curr_max_x
                            && center_y > curr_min_y
                            && center_y <= curr_max_y
                            && center_z > curr_min_z
                            && center_z <= curr_max_z
                        {
                            if curr_min_x == i16::MIN || curr_max_x == i16::MAX {
                                warn!("out of bounds quad? [{center_x}, {center_y}, {center_z}] not in [{min_x}, {min_y}, {min_z}] -> [{}, {}, {}]", min_x + size_x, min_y + size_y, min_z + size_z)
                            }
                            quads.extend_from_slice(quad);
                        }
                    }
                }

                // Filter out empty meshes
                if tris.is_empty() && quads.is_empty() {
                    continue;
                }

                // Create a mesh
                let mut combined_verts = Vec::new();
                combined_verts.extend(tris.iter());
                combined_verts.extend(quads.iter());
                meshes_to_add.push(MeshPSX {
                    verts: combined_verts,
                    n_triangles: tris.len() / 3,
                    n_quads: quads.len() / 4,
                    name: format!("{name} ({curr_min_x}, {curr_min_y}, {curr_min_z})"),
                })
            }
        }
    }
}

fn rgb_to_hsl(rgb: (f32, f32, f32)) -> (f32, f32, f32) {
    let (r, g, b) = rgb;
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    let delta = max - min;

    let l = (max + min) / 2.0;
    let s = if delta == 0.0 {
        0.0
    } else {
        delta / (1.0 - (2.0 * l - 1.0).abs())
    };

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };

    let h = if h < 0.0 { h + 360.0 } else { h };

    (h, s, l)
}

fn hsl_to_rgb(hsl: (f32, f32, f32)) -> (f32, f32, f32) {
    let (h, s, l) = hsl;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (r + m, g + m, b + m)
}
