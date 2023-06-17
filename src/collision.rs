use std::path::Path;

use tobj::LoadOptions;

use crate::psx_structs::{CollModelPSX, CollVertexPSX};

pub fn obj2col(input_obj: String, output_col: String) {
    let (models, _materials) = tobj::load_obj(
        input_obj,
        &LoadOptions {
            single_index: true,
            ..Default::default()
        },
    )
    .expect("Failed to OBJ load file");

    let mut triangles = Vec::<CollVertexPSX>::new();

    // Loop over every mesh in the model. We want to combine them all.
    for model in &models {
        println!("parsing {}", model.name);
        let mut curr_index = 0;

        let face_arities = match model.mesh.face_arities.is_empty() {
            false => model.mesh.face_arities.clone(),
            true => std::iter::repeat(3)
                .take(model.mesh.indices.len() / 3)
                .collect(),
        };

        for arity in &face_arities {
            let mut curr_primitive = Vec::<CollVertexPSX>::new();
            for in_face_index in curr_index as usize..(curr_index + arity) as usize {
                let index = model.mesh.indices[in_face_index] as usize;
                let vert = CollVertexPSX {
                    pos_x: (model.mesh.positions[index * 3 + 0] * -1024.0).clamp(-32768.0, 32767.0)
                        as i16,
                    pos_y: (model.mesh.positions[index * 3 + 1] * -1024.0).clamp(-32768.0, 32767.0)
                        as i16,
                    pos_z: (model.mesh.positions[index * 3 + 2] * 1024.0).clamp(-32768.0, 32767.0)
                        as i16,
                    terrain_id: 0,
                };
                curr_primitive.push(vert);
            }
            match arity {
                3 => {
                    for i in [0, 2, 1] {
                        triangles.push(curr_primitive[i]);
                    }
                },
                _ => println!("found polygon with more than 3 vertices! make sure the collision mesh only contains triangles."),
            };

            curr_index += arity;
        }
    }

    let collision_model_psx = CollModelPSX { verts: triangles };
    collision_model_psx.save(Path::new(&output_col));
}
