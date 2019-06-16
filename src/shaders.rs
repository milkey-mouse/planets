pub mod vertex {
    vulkano_shaders::shader!{
        ty: "vertex",
        path: "shaders/vertex.vert"
    }
}

pub mod fragment {
    vulkano_shaders::shader!{
        ty: "fragment",
        path: "shaders/fragment.frag"
    }
}
