use wgpu;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub const fn make_quad(width: u32, height: u32) -> [Vertex; 6] {
    [
        Vertex {
            position: [0., 0.],
            tex_coords: [0., 0.],
        },
        Vertex {
            position: [width as f32, 0.],
            tex_coords: [1., 0.],
        },
        Vertex {
            position: [0., height as f32],
            tex_coords: [0., 1.],
        },
        Vertex {
            position: [width as f32, 0.],
            tex_coords: [1., 0.],
        },
        Vertex {
            position: [width as f32, height as f32],
            tex_coords: [1., 1.],
        },
        Vertex {
            position: [0., height as f32],
            tex_coords: [0., 1.],
        },
    ]
}
