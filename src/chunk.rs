use bevy::{
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};

use crate::iters::Iter3d;

#[derive(Component, Clone)]
pub struct Chunk {
    pub points: Vec<f32>,
    pub size: usize,
}

impl Chunk {
    pub fn new(points: Vec<f32>, size: usize) -> Self {
        Self { points, size }
    }

    pub fn get(&self, pos: Vec3) -> f32 {
        self.points[self.index(pos)]
    }

    pub fn set(&mut self, pos: Vec3, value: f32) {
        let index = self.index(pos);
        self.points[index] = value;
    }

    pub fn new_iter_3d(size: u32) -> Iter3d {
        Iter3d::new(UVec3::ZERO, UVec3::new(size, size, size))
    }

    fn index(&self, pos: Vec3) -> usize {
        (pos.z as usize * self.size * self.size) + (pos.y as usize * self.size) + pos.x as usize
    }
}

#[derive(Component, Default, Clone)]
pub struct ChunkMesh {
    pub triangles: Vec<[Vec3; 3]>,
}

impl From<ChunkMesh> for Mesh {
    fn from(chunk: ChunkMesh) -> Self {
        // This tries to re-use vertices when they share a normal
        // if they have a different a normal it uses a different index.
        // This makes it possible to use face normals instead of vertex normals
        // while still using the smallest amount of vertices possible.

        fn face_normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
            (b - a).cross(c - a).normalize()
        }

        let mut indices = Vec::new();
        let mut vertices_normals = Vec::new();
        for [a, b, c] in chunk.triangles {
            let normal = face_normal(a, b, c);
            for vertex in [a, b, c] {
                // find a matching vertex/normal pair
                match vertices_normals
                    .iter()
                    .position(|&(v, n)| v == vertex && n == normal)
                {
                    Some(index) => indices.push(index as u32),
                    None => {
                        vertices_normals.push((vertex, normal));
                        indices.push(vertices_normals.len() as u32 - 1);
                    }
                }
            }
        }

        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut normals = Vec::new();

        for (vertex, normal) in &vertices_normals {
            positions.push([vertex.x, vertex.y, vertex.z]);
            uvs.push([0.0, 0.0]);
            normals.push([normal.x, normal.y, normal.z]);
        }

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
        mesh.set_indices(Some(Indices::U32(indices)));
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh
    }
}

/// Computes vertex normals which makes it possible to share the same vertex for multiple face
fn _compute_vertex_normals(vertices: &Vec<Vec3>, indices: &Vec<u32>) -> Vec<[f32; 3]> {
    let mut normals = vec![Vec3::ZERO; vertices.len()];

    // For each face, compute the face normal, and accumulate it into each vertex.
    for indices in indices.chunks(3) {
        if let [a, b, c] = indices {
            let edge_ab = vertices[*b as usize] - vertices[*a as usize];
            let edge_ac = vertices[*c as usize] - vertices[*a as usize];

            // The cross product is perpendicular to both input vectors (normal to the plane).
            // Flip the argument order if you need the opposite winding.
            let normal = edge_ab.cross(edge_ac);

            // Don't normalize this vector just yet. Its magnitude is proportional to the
            // area of the triangle (times 2), so this helps ensure tiny/skinny triangles
            // don't have an outsized impact on the final normal per vertex.
            normals[*a as usize] += normal;
            normals[*b as usize] += normal;
            normals[*c as usize] += normal;
        }
    }

    // Finally, normalize all the sums to get a unit-length, area-weighted average.
    for normal in normals.iter_mut() {
        *normal = normal.normalize();
    }

    normals.iter().map(|n| [n.x, n.y, n.z]).collect()
}
