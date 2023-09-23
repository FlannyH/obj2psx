use std::collections::HashMap;
use glam::Vec3;
use crate::MeshGridEntry;

#[derive(Copy, Clone)]
struct Triangle {
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
}

#[derive(Copy, Clone)]
struct Quad {
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
    v3: Vec3,
}

#[derive(Copy, Clone)]
struct Plane {
    position: Vec3,
    normal: Vec3,
}

struct BspTree {
    nodes: Vec::<BspNode>,
    indices: Vec::<u32>,
    polygons: Vec::<Polygon>,
}

struct BspNodeParent {
    split_plane: Plane,
    child_node_index: u32,
    index_first_polygon: u32,
    polygon_count: u32,
}

struct BspNodeLeaf {
    index_first_polygon: u32,
    polygon_count: u32,
}

enum BspNode {
    Parent(BspNodeParent),
    Leaf(BspNodeLeaf),
}

#[derive(Copy, Clone)]
enum Polygon {
    Triangle(Triangle),
    Quad(Quad),
}

pub fn split_bsp(mesh_map: HashMap<String, MeshGridEntry>, poly_limit: u32) {
    let mut polygons = Vec::<Polygon>::new();

    // Get all polygons in one big buffer
    for value in mesh_map.values() {
        for tri in value.triangles.chunks(3) {
            polygons.push(Polygon::Triangle(Triangle {
                v0: Vec3 {x: tri[0].pos_x as f32, y: tri[0].pos_y as f32, z: tri[0].pos_z as f32},
                v1: Vec3 {x: tri[1].pos_x as f32, y: tri[1].pos_y as f32, z: tri[1].pos_z as f32},
                v2: Vec3 {x: tri[2].pos_x as f32, y: tri[2].pos_y as f32, z: tri[2].pos_z as f32},
            }))
        }
        for quad in value.quads.chunks(4) {
            polygons.push(Polygon::Quad(Quad {
                v0: Vec3 {x: quad[0].pos_x as f32, y: quad[0].pos_y as f32, z: quad[0].pos_z as f32},
                v1: Vec3 {x: quad[1].pos_x as f32, y: quad[1].pos_y as f32, z: quad[1].pos_z as f32},
                v2: Vec3 {x: quad[2].pos_x as f32, y: quad[2].pos_y as f32, z: quad[2].pos_z as f32},
                v3: Vec3 {x: quad[3].pos_x as f32, y: quad[3].pos_y as f32, z: quad[3].pos_z as f32},
            }))
        }
    }

    // Create BSP tree
    let mut bsp_tree = BspTree {
        nodes: Vec::new(),
        indices: (0..polygons.len() as u32).collect(),
        polygons,
    };
    bsp_tree.nodes.push(BspNode::Parent(BspNodeParent { split_plane: Plane { position: Vec3::ZERO, normal: Vec3::ZERO }, child_node_index: 1, index_first_polygon: 0, polygon_count: bsp_tree.polygons.len() as _ }));

    let mut node_subdivision_queue = Vec::<u32>::new();

    loop {
        match node_subdivision_queue.pop() {
            Some(node_index) => {
                subdivide(&mut bsp_tree, node_index, &mut node_subdivision_queue, poly_limit);
            },
            None => break,
        }
    }

    // todo: collect all leaf nodes and return it as Vec of meshes
}

fn subdivide(bsp: &mut BspTree, node_index: u32, stack: &mut Vec::<u32>, poly_limit: u32) {
    let mut node = match &mut bsp.nodes[node_index as usize] {
        BspNode::Parent(n) => n,
        BspNode::Leaf(_) => unreachable!(), // If it's a leaf, we don't add it to the queue, so this never happens
    };
    
    // If this node reached below the polygon limit, make this a leaf
    if node.polygon_count < poly_limit {
        bsp.nodes[node_index as usize] = BspNode::Leaf(BspNodeLeaf {
            index_first_polygon: node.index_first_polygon,
            polygon_count: node.polygon_count,
        });
        return;
    }

    // Find the split plane that creates the most equal split in terms of polygon count on either side
    node.split_plane = find_split_plane(&bsp.polygons, &bsp.indices, 0, bsp.polygons.len() as u32);

    // Partition the polygons to front and behind the plane
    let split_index = partition(&mut bsp.indices, &bsp.polygons, node.split_plane, node.index_first_polygon, node.polygon_count);
    let start1 = node.index_first_polygon;
    let start2 = split_index;
    let count1 = split_index - start1;
    let count2 = node.index_first_polygon + node.polygon_count - split_index;

    // Create left node
    stack.push(bsp.nodes.len() as u32);
    bsp.nodes.push(BspNode::Parent(BspNodeParent {
        split_plane: find_split_plane(&bsp.polygons, &bsp.indices, start1, count1),
        child_node_index: 0,
        index_first_polygon : start1,
        polygon_count: count1,
    }));

    // Create right node
    stack.push(bsp.nodes.len() as u32);
    bsp.nodes.push(BspNode::Parent(BspNodeParent {
        split_plane: find_split_plane(&bsp.polygons, &bsp.indices, start2, count2),
        child_node_index: 0,
        index_first_polygon : start2,
        polygon_count: count2,
    }));
}

// Returns split index
fn partition(indices: &mut Vec::<u32>, polygons: &Vec::<Polygon>, split_plane: Plane, start: u32, count: u32) -> u32 {
    let mut i = start;
    let mut j = start + count - 1;
    while i <= j {
        // Get polygon center
        let center = match &polygons[indices[i as usize] as usize] {
            Polygon::Triangle(tri) => (tri.v0 + tri.v1 + tri.v2) / 3.0,
            Polygon::Quad(quad) => (quad.v0 + quad.v1 + quad.v2 + quad.v3) / 3.0,
        };

        // If the current polygon is in front of the triangle
        if (center - split_plane.position).dot(split_plane.normal) > 0.0 {
            (indices[i as usize], indices[j as usize]) =
                (indices[j as usize], indices[i as usize]);
            j -= 1;
        } else {
            i += 1;
        }
    }

    return i;
}

// Takes a bunch of polygons and returns a split plane
fn find_split_plane(polygons: &Vec::<Polygon>, indices: &Vec::<u32>, start: u32, count: u32) -> Plane {
    let mut best_split_polygon_score = 0.0;
    let mut best_split_plane = None;
    //for split_polygon in polygons {
    for index in start..(start + count) {
        let split_polygon = polygons[indices[index as usize] as usize];

        // Make plane from polygon
        let plane = match split_polygon {
            Polygon::Triangle(tri) => {
                Plane {
                    position: tri.v0,
                    normal: (tri.v1-tri.v0).cross(tri.v2-tri.v0),
                }
            },
            Polygon::Quad(quad) => {
                Plane {
                    position: quad.v0,
                    normal: (quad.v1-quad.v0).cross(quad.v2-quad.v0),
                }
            }
        };
        
        // Test other polygons against this plane
        let mut n_positive = 0;
        for compare_polygon in polygons {
            let compare_position = match compare_polygon {
                Polygon::Triangle(tri) => (tri.v0 + tri.v1 + tri.v2) / 3.0,
                Polygon::Quad(quad) => (quad.v0 + quad.v1 + quad.v2 + quad.v3) / 3.0,
            };

            if (compare_position - plane.position).dot(plane.normal) > 0.0 {
                n_positive += 1;
            }
        }

        // Give it a score from 0.0 to 1.0
        let score = n_positive as f32 / polygons.len() as f32; // 0.0 is bad, 1.0 is bad, 0.5 is good
        let score = (score - 0.5).abs(); // 0.0 is good, 1.0 is bad
        let score = 1.0 - score; // 1.0 is good, 0.0 is bad

        // If the score is better, update it
        if score > best_split_polygon_score {
            best_split_polygon_score = score;
            best_split_plane = Some(plane);
        }
    }

    best_split_plane.unwrap()
}
