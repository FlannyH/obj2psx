use crate::{psx_structs::VertexPSX, MeshGridEntry};
use glam::Vec3;
use std::collections::HashMap;

#[derive(Copy, Clone)]
pub struct Triangle {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
}

#[derive(Copy, Clone)]
pub struct Quad {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
    pub v3: Vec3,
}

#[derive(Copy, Clone)]
struct Plane {
    position: Vec3,
    normal: Vec3,
}

struct BspTree<'a> {
    nodes: Vec<BspNode>,
    indices: Vec<u32>,
    polygons: Vec<Polygon<'a>>,
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
pub enum Polygon<'a> {
    Triangle(u32, Triangle, &'a [VertexPSX]),
    Quad(u32, Quad, &'a [VertexPSX]),
}

pub fn split_bsp(mesh_map: HashMap<String, MeshGridEntry>, poly_limit: u32) -> Vec<MeshGridEntry> {
    let mut polygons = Vec::<Polygon>::new();

    // Get all polygons in one big buffer
    let mut i = 0;
    for value in mesh_map.values() {
        for tri in value.triangles.chunks(3) {
            polygons.push(Polygon::Triangle(
                i,
                Triangle {
                    v0: Vec3 {
                        x: tri[0].pos_x as f32,
                        y: tri[0].pos_y as f32,
                        z: tri[0].pos_z as f32,
                    },
                    v1: Vec3 {
                        x: tri[1].pos_x as f32,
                        y: tri[1].pos_y as f32,
                        z: tri[1].pos_z as f32,
                    },
                    v2: Vec3 {
                        x: tri[2].pos_x as f32,
                        y: tri[2].pos_y as f32,
                        z: tri[2].pos_z as f32,
                    },
                },
                tri,
            ));
            i += 3;
        }
        for quad in value.quads.chunks(4) {
            polygons.push(Polygon::Quad(
                i,
                Quad {
                    v0: Vec3 {
                        x: quad[0].pos_x as f32,
                        y: quad[0].pos_y as f32,
                        z: quad[0].pos_z as f32,
                    },
                    v1: Vec3 {
                        x: quad[1].pos_x as f32,
                        y: quad[1].pos_y as f32,
                        z: quad[1].pos_z as f32,
                    },
                    v2: Vec3 {
                        x: quad[2].pos_x as f32,
                        y: quad[2].pos_y as f32,
                        z: quad[2].pos_z as f32,
                    },
                    v3: Vec3 {
                        x: quad[3].pos_x as f32,
                        y: quad[3].pos_y as f32,
                        z: quad[3].pos_z as f32,
                    },
                },
                quad,
            ));
            i += 4;
        }
    }

    // Create BSP tree
    let mut bsp_tree = BspTree {
        nodes: Vec::new(),
        indices: (0..polygons.len() as u32).collect(),
        polygons,
    };
    bsp_tree.nodes.push(BspNode::Parent(BspNodeParent {
        split_plane: Plane {
            position: Vec3::ZERO,
            normal: Vec3::ZERO,
        },
        child_node_index: 1,
        index_first_polygon: 0,
        polygon_count: bsp_tree.polygons.len() as _,
    }));

    let mut node_queue = Vec::<(u32 /*node_index*/, u32 /*rec_depth*/)>::new();
    node_queue.push((0, 0));

    let mut meshes = Vec::<MeshGridEntry>::new();

    while let Some((node_index, rec_depth)) = node_queue.pop() {
        subdivide(
            &mut bsp_tree,
            node_index,
            rec_depth,
            &mut node_queue,
            poly_limit,
            &mut meshes,
        );
    }

    meshes
}

fn subdivide(
    bsp: &mut BspTree,
    node_index: u32,
    rec_depth: u32,
    stack: &mut Vec<(u32, u32)>,
    poly_limit: u32,
    mesh_output: &mut Vec<MeshGridEntry>,
) {
    let node = match &mut bsp.nodes[node_index as usize] {
        BspNode::Parent(n) => n,
        BspNode::Leaf(_) => unreachable!(), // If it's a leaf, we don't add it to the queue, so this never happens
    };

    // Find the split plane that creates the most equal split in terms of polygon count on either side
    node.split_plane = find_split_plane(
        &bsp.polygons,
        &bsp.indices,
        node.index_first_polygon,
        node.polygon_count,
    );

    // Partition the polygons to front and behind the plane
    let split_index = partition(
        &mut bsp.indices,
        &bsp.polygons,
        node.split_plane,
        node.index_first_polygon,
        node.polygon_count,
    );
    let start1 = node.index_first_polygon;
    let start2 = split_index;
    let count1 = split_index - start1;
    let count2 = node.index_first_polygon + node.polygon_count - split_index;

    // If this node reached below the polygon limit, or one of the split plane counts was 0
    if (node.polygon_count < poly_limit) || (count1 == 0) || (count2 == 0) {
        let start = node.index_first_polygon;
        let end = start + node.polygon_count;

        // Make this a leaf node
        bsp.nodes[node_index as usize] = BspNode::Leaf(BspNodeLeaf {
            index_first_polygon: node.index_first_polygon,
            polygon_count: node.polygon_count,
        });

        // Create new MeshGridEntry and put all the polygons in it
        let mut mesh = MeshGridEntry {
            triangles: Vec::new(),
            quads: Vec::new(),
        };
        for i_index in start..end {
            let i_polygon = bsp.indices[i_index as usize];
            let polygon = bsp.polygons[i_polygon as usize];
            match polygon {
                Polygon::Triangle(_, _, vertices) => mesh.triangles.extend_from_slice(vertices),
                Polygon::Quad(_, _, vertices) => mesh.quads.extend_from_slice(vertices),
            }
        }

        mesh_output.push(mesh);

        return;
    }

    // Create left node
    stack.push((bsp.nodes.len() as u32, rec_depth + 1));
    bsp.nodes.push(BspNode::Parent(BspNodeParent {
        split_plane: find_split_plane(&bsp.polygons, &bsp.indices, start1, count1),
        child_node_index: 0,
        index_first_polygon: start1,
        polygon_count: count1,
    }));

    // Create right node
    stack.push((bsp.nodes.len() as u32, rec_depth + 1));
    bsp.nodes.push(BspNode::Parent(BspNodeParent {
        split_plane: find_split_plane(&bsp.polygons, &bsp.indices, start2, count2),
        child_node_index: 0,
        index_first_polygon: start2,
        polygon_count: count2,
    }));
}

// Returns split index
fn partition(
    indices: &mut [u32],
    polygons: &[Polygon],
    split_plane: Plane,
    start: u32,
    count: u32,
) -> u32 {
    let mut i = start;
    let mut j = start + count - 1;
    while i <= j {
        // Get polygon center
        let center = match &polygons[indices[i as usize] as usize] {
            Polygon::Triangle(_, tri, _) => (tri.v0 + tri.v1 + tri.v2) / 3.0,
            Polygon::Quad(_, quad, _) => (quad.v0 + quad.v1 + quad.v2 + quad.v3) / 3.0,
        };

        // If the current polygon is in front of the triangle
        if (center - split_plane.position).dot(split_plane.normal) > 0.0 {
            (indices[i as usize], indices[j as usize]) = (indices[j as usize], indices[i as usize]);
            j -= 1;
        } else {
            i += 1;
        }
    }

    i
}

// Takes a bunch of polygons and returns a split plane
fn find_split_plane(polygons: &[Polygon], indices: &[u32], start: u32, count: u32) -> Plane {
    // Find center of all polygons
    let mut center = Vec3::ZERO;
    for i in start..(start + count) {
        let polygon = polygons[indices[i as usize] as usize];
        center += match polygon {
            Polygon::Triangle(_, tri, _) => (tri.v0 + tri.v1 + tri.v2) / 3.0,
            Polygon::Quad(_, quad, _) => (quad.v0 + quad.v1 + quad.v2 + quad.v3) / 3.0,
        }
    }
    center /= count as f32;

    // Find closest and furthest polygon to center
    let mut closest_distance = f32::INFINITY;
    let mut closest_polygon_center = Vec3::ZERO;
    let mut furthest_distance = 0.0;
    let mut furthest_polygon_center = Vec3::ZERO;
    for i in start..(start + count) {
        let polygon = polygons[indices[i as usize] as usize];
        let curr_center = match polygon {
            Polygon::Triangle(_, tri, _) => (tri.v0 + tri.v1 + tri.v2) / 3.0,
            Polygon::Quad(_, quad, _) => (quad.v0 + quad.v1 + quad.v2 + quad.v3) / 3.0,
        };
        let distance = curr_center.distance(center);
        if distance < closest_distance {
            closest_distance = distance;
            closest_polygon_center = curr_center;
        }
        if distance > furthest_distance {
            furthest_distance = distance;
            furthest_polygon_center = curr_center;
        }
    }

    // Create plane from that
    Plane {
        position: closest_polygon_center,
        normal: (furthest_polygon_center - closest_polygon_center).normalize(),
    }
}
