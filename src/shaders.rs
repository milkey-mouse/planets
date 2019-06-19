pub mod particle_vert {
    vulkano_shaders::shader!{
        ty: "vertex",
        path: "shaders/particle.vert"
    }

    #[derive(Debug, Clone, Default)]
    pub struct Vertex {
        pub position: [f32; 2],
        pub velocity: [f32; 2],
    }
    vulkano::impl_vertex!(Vertex, position, velocity);
}

pub mod particle_frag {
    vulkano_shaders::shader!{
        ty: "fragment",
        path: "shaders/particle.frag"
    }
}
