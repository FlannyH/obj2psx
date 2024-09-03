use std::path::Path;

use glam::I64Vec3;
use tobj::LoadOptions;

use crate::{psx_structs::{CollModelPSX, CollVertexPSX, NavGraphNode}, renderer::Renderer};

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

    let bvh = CollBvh::construct(&triangles);

    let mut nav_graph_nodes = Vec::<NavGraphNode>::new();
    for (center, primitive) in bvh.centers.iter().zip(&bvh.primitives) {
        if primitive.normal.y < 2048 {
            continue;
        }

        let center_x16 = (center.x / 4096) as i16;
        let center_y16 = (center.y / 4096) as i16;
        let center_z16 = (center.z / 4096) as i16;

        let value = NavGraphNode {
            pos_x: center_x16,
            pos_y: center_y16,
            pos_z: center_z16,
            neighbors: [0, 0, 0, 0], // We fill this in below
        };
        nav_graph_nodes.push(value);
    }

    let mut renderer = Renderer::new();

    let mut triangles_without_floor = vec![];
    for triangle in triangles.chunks(3) {
        let v0 = glam::vec3(triangle[0].pos_x as f32, triangle[0].pos_y as f32, triangle[0].pos_z as f32);
        let v1 = glam::vec3(triangle[1].pos_x as f32, triangle[1].pos_y as f32, triangle[1].pos_z as f32);
        let v2 = glam::vec3(triangle[2].pos_x as f32, triangle[2].pos_y as f32, triangle[2].pos_z as f32);
        let edge_0_2 = v2 - v0;
        let edge_0_1 = v1 - v0;
        let normal = edge_0_2.cross(edge_0_1).normalize_or_zero();

        if normal.y > -0.5 {
            triangles_without_floor.push(triangle[0]);
            triangles_without_floor.push(triangle[1]);
            triangles_without_floor.push(triangle[2]);
        }
    }
    renderer.upload_mesh_to_gpu(&triangles_without_floor);

    // Naive approach to finding neighbors (closest cells)
    let mut max = 0.0f32;
    for node1_index in 0..nav_graph_nodes.len() {
        // Add to circular buffer any time the value is lower than the last
        let mut closest_distances = [f32::INFINITY, f32::INFINITY, f32::INFINITY, f32::INFINITY];
        let mut closest_indices = [0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF]; // Initialize to invalid value 0xFFFF, so we know when to end early if there's less neighbors
        let mut circular_buffer_index = 0;

        for node2_index in 0..nav_graph_nodes.len() {
            if node1_index == node2_index {
                continue;
            }

            let node1 = &nav_graph_nodes[node1_index];
            let node2 = &nav_graph_nodes[node2_index];
            let a = glam::vec3(node1.pos_x as f32, node1.pos_y as f32 + 8.0, node1.pos_z as f32);
            let b = glam::vec3(node2.pos_x as f32, node2.pos_y as f32 + 8.0, node2.pos_z as f32);
            let distance = a.distance(b);

            if distance >= 128.0 {
                continue;
            }

            if distance >= closest_distances[circular_buffer_index] {
                continue;
            }

            if renderer.is_path_occupied(a * 8.0, b * 8.0, 24.0) {
                continue;
            }

            closest_distances[circular_buffer_index] = distance;
            closest_indices[circular_buffer_index] = node2_index;
            circular_buffer_index += 1;
            circular_buffer_index %= closest_distances.len();
        }

        for (i, closest_index) in closest_indices.iter().enumerate() {
            nav_graph_nodes[node1_index].neighbors[i] = *closest_index as u16;
            if closest_distances[i] != f32::INFINITY {
                max = max.max(closest_distances[i]);
            }
        }
    }

    let collision_model_psx = CollModelPSX {
        triangles: bvh.primitives,
        nodes: bvh.nodes,
        indices: bvh.indices,
        nav_graph_nodes,
    };

    collision_model_psx.save(Path::new(&output_col));
}

pub struct CollTrianglePSX {
    pub v0: glam::IVec3,
    pub v1: glam::IVec3,
    pub v2: glam::IVec3,
    pub normal: glam::IVec3,

    // Will be exported separately
    pub terrain_id: u8,
}

pub struct Aabb {
    pub min: glam::IVec3,
    pub max: glam::IVec3,
}
pub struct BvhNode {
    pub bounds: Aabb, // Axis aligned bounding box around all primitives inside this node
    pub left_first: u16, // If this is a leaf, this is the index of the first primitive, otherwise, this is the index of the first of two child nodes
    pub primitive_count: u16, // If this value is not 0, this is a leaf node
}

enum Axis {
    X,
    Y,
    Z,
}

struct CollBvh {
    primitives: Vec<CollTrianglePSX>,
    indices: Vec<u16>,
    nodes: Vec<BvhNode>,

    // Intermediates, won't get stored in the output file
    centers: Vec<glam::IVec3>,
    node_pointer: usize,
}

const COL_SCALE: i32 = 512;

impl CollBvh {
    pub fn construct(vertices: &Vec<CollVertexPSX>) -> CollBvh {
        let mut bvh = CollBvh {
            primitives: vec![],
            indices: vec![],
            nodes: vec![],
            centers: vec![],
            node_pointer: 0,
        };

        // Get primitives and their center points
        for triangle in vertices.chunks_exact(3) {
            let v0 = glam::IVec3::new(
                triangle[0].pos_x as i32 * -COL_SCALE,
                triangle[0].pos_y as i32 * -COL_SCALE,
                triangle[0].pos_z as i32 * -COL_SCALE,
            );
            let v1 = glam::IVec3::new(
                triangle[1].pos_x as i32 * -COL_SCALE,
                triangle[1].pos_y as i32 * -COL_SCALE,
                triangle[1].pos_z as i32 * -COL_SCALE,
            );
            let v2 = glam::IVec3::new(
                triangle[2].pos_x as i32 * -COL_SCALE,
                triangle[2].pos_y as i32 * -COL_SCALE,
                triangle[2].pos_z as i32 * -COL_SCALE,
            );

            // Calculate normal
            let edge_0_2 = (v2 - v0).as_vec3();
            let edge_0_1 = (v1 - v0).as_vec3();
            let normal = edge_0_2.cross(edge_0_1).normalize_or_zero()
                * glam::Vec3::new(-4096.0, -4096.0, -4096.0); // Fixed point 1.0 = 4096;

            // Store in Bvh
            bvh.primitives.push(CollTrianglePSX {
                v0,
                v1,
                v2,
                normal: normal.as_ivec3(),
                terrain_id: 0,
            }); // todo: unhardcode terrain id
            bvh.centers.push((v0 + v1 + v2) / glam::IVec3::new(3, 3, 3));
        }

        // Create index array
        bvh.indices = (0..bvh.primitives.len() as u16).collect();

        // Create root node
        bvh.nodes.push(BvhNode {
            bounds: Aabb {
                min: glam::IVec3 { x: 0, y: 0, z: 0 },
                max: glam::IVec3 { x: 0, y: 0, z: 0 },
            },
            left_first: 0,
            primitive_count: bvh.primitives.len() as u16,
        });

        // Create empty dummy node so each pair is aligned to a multiple of 2
        bvh.nodes.push(BvhNode {
            bounds: Aabb {
                min: glam::IVec3 { x: 0, y: 0, z: 0 },
                max: glam::IVec3 { x: 0, y: 0, z: 0 },
            },
            left_first: 0,
            primitive_count: bvh.primitives.len() as u16,
        });

        bvh.subdivide(0, 0);

        bvh
    }

    fn get_bounds(&self, first: u16, count: u16) -> Aabb {
        let mut result = Aabb {
            min: glam::IVec3::MAX,
            max: glam::IVec3::MIN,
        };

        for i in 0..count {
            let triangle = &self.primitives[self.indices[(first + i) as usize] as usize];
            result.max = result
                .max
                .max(triangle.v0.max(triangle.v1.max(triangle.v2)));
            result.min = result
                .min
                .min(triangle.v0.min(triangle.v1.min(triangle.v2)));
        }

        result
    }

    fn subdivide(&mut self, node_index: usize, recursion_depth: usize) {
        let leaf_display = || {
            let debug_display_recursion_depth = false;
            if debug_display_recursion_depth {
                print!("{recursion_depth:3}");
                for _ in 0..recursion_depth {
                    print!("-");
                }
                print!(" ");
            }
        };

        // Determine AABB for primitives in array
        self.nodes[node_index].bounds = self.get_bounds(
            self.nodes[node_index].left_first,
            self.nodes[node_index].primitive_count,
        );

        if self.nodes[node_index].primitive_count < 3 {
            leaf_display();
            return;
        }

        // todo: implement surface area heuristic for free performance

        // Get the average position of all the primitives
        let node = &self.nodes[node_index];
        let mut avg = glam::I64Vec3::new(0, 0, 0);
        for i in node.left_first..(node.left_first + node.primitive_count) {
            avg += self.primitives[self.indices[i as usize] as usize]
                .v0
                .as_i64vec3();
            avg += self.primitives[self.indices[i as usize] as usize]
                .v1
                .as_i64vec3();
            avg += self.primitives[self.indices[i as usize] as usize]
                .v2
                .as_i64vec3();
        }
        avg /= glam::I64Vec3::new(
            node.primitive_count as i64 * 3,
            node.primitive_count as i64 * 3,
            node.primitive_count as i64 * 3,
        );

        // Determine split axis - choose biggest axis
        let mut split_axis = Axis::X;
        let mut split_pos = 0;

        let size = node.bounds.max - node.bounds.min;

        if size.x > size.y && size.x > size.z {
            split_axis = Axis::X;
            split_pos = avg.x;
        }

        if size.y > size.x && size.y > size.z {
            split_axis = Axis::Y;
            split_pos = avg.y;
        }

        if size.z > size.x && size.z > size.y {
            split_axis = Axis::Z;
            split_pos = avg.z;
        }

        // Partition the index array, and get the split position
        let split_index = {
            let mut i = node.left_first;
            for j in (node.left_first)..(node.left_first + node.primitive_count) {
                // Get min and max of the axis we want
                let prim = &self.primitives[self.indices[j as usize] as usize];
                let bounds = Aabb {
                    min: prim.v0.min(prim.v1.min(prim.v2)),
                    max: prim.v0.max(prim.v1.max(prim.v2)),
                };

                // Get center
                let center =
                    (bounds.min.as_i64vec3() + bounds.max.as_i64vec3()) / I64Vec3::new(2, 2, 2);
                let center_point = match split_axis {
                    Axis::X => center.as_ivec3().x,
                    Axis::Y => center.as_ivec3().y,
                    Axis::Z => center.as_ivec3().z,
                };

                // Swap primitives that are on the wrong sides of the pivot
                if (center_point > split_pos as i32) && (j != i) {
                    self.indices.swap(i as usize, j as usize);
                    i += 1;
                }
            }
            i
        };

        // If splitIndex is at the end of the array, we've reached a dead end, so stop here
        if split_index == (node.left_first + node.primitive_count) {
            leaf_display();
            return;
        }

        // If splitIndex and the start are the same, we've reached a dead end, so stop here
        if split_index == node.left_first {
            leaf_display();
            return;
        }

        // Save the start index of this node
        let start_index = node.left_first;

        // Create child nodes
        let primitive_count = node.primitive_count;
        self.nodes[node_index].left_first = self.nodes.len() as _;

        // Left
        self.nodes.push(BvhNode {
            bounds: Aabb {
                max: glam::IVec3::new(0, 0, 0),
                min: glam::IVec3::new(0, 0, 0),
            },
            left_first: start_index,
            primitive_count: split_index - start_index,
        });

        // Right
        self.nodes.push(BvhNode {
            bounds: Aabb {
                max: glam::IVec3::new(0, 0, 0),
                min: glam::IVec3::new(0, 0, 0),
            },
            left_first: split_index,
            primitive_count: start_index + primitive_count - split_index,
        });

        self.subdivide(
            self.nodes[node_index].left_first as usize + 0,
            recursion_depth + 1,
        );
        self.subdivide(
            self.nodes[node_index].left_first as usize + 1,
            recursion_depth + 1,
        );

        self.nodes[node_index].primitive_count = 0;
    }

    fn partition(
        primitives: &[CollTrianglePSX],
        indices: &mut [u16],
        axis: Axis,
        pivot: i32,
        start: u16,
        count: u16,
        split_index: &mut u16,
    ) {
        let mut i = start;
        for j in start..(start + count) {
            // Get min and max of the axis we want
            let prim = &primitives[indices[j as usize] as usize];
            let bounds = Aabb {
                min: prim.v0.min(prim.v1.min(prim.v2)),
                max: prim.v0.max(prim.v1.max(prim.v2)),
            };

            // Get center
            let center =
                (bounds.min.as_i64vec3() + bounds.max.as_i64vec3()) / I64Vec3::new(2, 2, 2);
            let center_point = match axis {
                Axis::X => center.as_ivec3().x,
                Axis::Y => center.as_ivec3().y,
                Axis::Z => center.as_ivec3().z,
            };

            // Swap primitives that are on the wrong sides of the pivot
            if (center_point > pivot) && (j != i) {
                indices.swap(i as usize, j as usize);
                i += 1;
            }
        }

        *split_index = i;
    }
}
