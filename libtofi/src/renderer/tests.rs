//! Unit tests for `libtofi::renderer`.
//!
//! These tests create an in-memory ARGB buffer (no Wayland display needed) and
//! verify that [`Renderer`] writes non-zero pixels into it after `draw_hello`.

#[cfg(feature = "renderer")]
mod renderer_tests {
    use crate::renderer::Renderer;

    /// 1× scale (scale_numerator = 120): logical == physical, "hello" painted.
    #[test]
    fn draw_hello_writes_pixels() {
        let width: u32 = 400;
        let height: u32 = 200;
        let mut buf = vec![0u8; (width * height * 4) as usize];

        // SAFETY: buf is valid for the lifetime of the Renderer below.
        let renderer = unsafe { Renderer::create_for_data(buf.as_mut_ptr(), width, height, 120) }
            .expect("Renderer::create_for_data should succeed");

        assert_eq!(renderer.logical_width, width);
        assert_eq!(renderer.logical_height, height);

        renderer.draw_hello();
        renderer.flush();
        drop(renderer);

        // After draw_hello the buffer should not be all-zero (some pixels painted).
        let all_zero = buf.iter().all(|&b| b == 0);
        assert!(
            !all_zero,
            "draw_hello should paint at least one non-zero pixel"
        );
    }

    /// 2× integer scale (scale_numerator = 240): logical dims are half physical.
    #[test]
    fn logical_dims_halved_at_2x_scale() {
        let phys_w: u32 = 800;
        let phys_h: u32 = 400;
        let mut buf = vec![0u8; (phys_w * phys_h * 4) as usize];

        let renderer = unsafe { Renderer::create_for_data(buf.as_mut_ptr(), phys_w, phys_h, 240) }
            .expect("2× Renderer should succeed");

        assert_eq!(renderer.logical_width, 400);
        assert_eq!(renderer.logical_height, 200);
    }

    /// Fractional 1.5× scale (scale_numerator = 180): logical = physical / 1.5.
    #[test]
    fn logical_dims_fractional_scale() {
        let phys_w: u32 = 600;
        let phys_h: u32 = 300;
        let mut buf = vec![0u8; (phys_w * phys_h * 4) as usize];

        let renderer = unsafe { Renderer::create_for_data(buf.as_mut_ptr(), phys_w, phys_h, 180) }
            .expect("1.5× Renderer should succeed");

        // 600 / 1.5 = 400, 300 / 1.5 = 200 (rounded).
        assert_eq!(renderer.logical_width, 400);
        assert_eq!(renderer.logical_height, 200);
    }
}
