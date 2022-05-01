use bevy::{prelude::*, render::mesh::PrimitiveTopology};

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

    pub fn new_iter(size: u32) -> Iter3d {
        Iter3d::new(UVec3::ZERO, UVec3::new(size - 2, size - 2, size - 2))
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
        // TODO re-use existing vertices
        let mut vertices = Vec::with_capacity(chunk.triangles.len() * 3);
        for triangle_vertices in chunk.triangles {
            for vertex in triangle_vertices {
                vertices.push(([vertex.x, vertex.y, vertex.z], [0.0, 0.0]));
            }
        }

        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        vertices.reverse();
        for (position, uv) in &vertices {
            positions.push(*position);
            uvs.push(*uv);
        }

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.compute_flat_normals();
        mesh
    }
}
