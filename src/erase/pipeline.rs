use crate::erase::mask::feather_mask;
use crate::erase::models::ModelPaths;
use crate::error::MartiniError;
use image::{DynamicImage, GrayImage, Rgba};

pub struct InpaintPipeline {}

impl InpaintPipeline {
    pub fn load(_paths: &ModelPaths) -> Result<Self, MartiniError> {
        Ok(Self {})
    }

    pub fn inpaint(
        &self,
        image: &DynamicImage,
        _mask: &GrayImage,
        _steps: usize,
    ) -> Result<DynamicImage, MartiniError> {
        Ok(image.clone())
    }

    /// Blends an inpainted crop patch back into the original image using the feathered mask
    pub fn blend_result(
        original: &DynamicImage,
        inpainted_patch: &DynamicImage,
        mask: &GrayImage,
        crop: (u32, u32, u32, u32),
    ) -> Result<DynamicImage, MartiniError> {
        let (crop_x, crop_y, crop_w, crop_h) = crop;
        let mut result = original.to_rgba8();
        let patch_rgba = inpainted_patch.to_rgba8();

        // Feather mask slightly to ensure smooth boundaries
        let feathered = feather_mask(mask, 2);

        for py in 0..crop_h {
            for px in 0..crop_w {
                let ox = crop_x + px;
                let oy = crop_y + py;

                if ox < result.width()
                    && oy < result.height()
                    && px < patch_rgba.width()
                    && py < patch_rgba.height()
                {
                    let mask_val = feathered.get_pixel(ox, oy)[0] as f32 / 255.0;

                    if mask_val > 0.0 {
                        let orig_px = result.get_pixel(ox, oy);
                        let patch_px = patch_rgba.get_pixel(px, py);

                        let r = ((1.0 - mask_val) * orig_px[0] as f32
                            + mask_val * patch_px[0] as f32)
                            .round() as u8;
                        let g = ((1.0 - mask_val) * orig_px[1] as f32
                            + mask_val * patch_px[1] as f32)
                            .round() as u8;
                        let b = ((1.0 - mask_val) * orig_px[2] as f32
                            + mask_val * patch_px[2] as f32)
                            .round() as u8;
                        let a = ((1.0 - mask_val) * orig_px[3] as f32
                            + mask_val * patch_px[3] as f32)
                            .round() as u8;

                        result.put_pixel(ox, oy, Rgba([r, g, b, a]));
                    }
                }
            }
        }

        Ok(DynamicImage::ImageRgba8(result))
    }
}
