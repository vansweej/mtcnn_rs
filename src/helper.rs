use image::imageops::*;
use image::*;
use ndarray::prelude::*;
use show_image::*;
use std::cmp;
use std::time::Duration;

pub fn display_image(image: impl ImageData) {
    // Create a window and display the image.
    let options = WindowOptions {
        name: "image".to_string(),
        size: [384, 710],
        resizable: true,
        preserve_aspect_ratio: false,
    };

    let window = make_window_full(options).unwrap();
    window.set_image(image, "image-001").unwrap();

    // Print keyboard events until Escape is pressed, then exit.
    // If the user closes the window, wait_key() will return an error and the loop also exits.
    while let Ok(event) = window.wait_key(Duration::from_millis(100)) {
        if let Some(event) = event {
            if event.key == KeyCode::Escape {
                break;
            }
        }
    }

    // Make sure all background tasks are stopped cleanly.
    //show_image::stop().unwrap();
}

// Some numpy functions
pub fn maximum<A, D>(
    num: &A,
    num_array: Array<A, D>,
) -> ArrayBase<ndarray::OwnedRepr<A>, Dim<[usize; 1]>>
where
    A: std::cmp::PartialOrd + std::marker::Copy,
    D: Dimension,
{
    num_array
        .iter()
        .map(|v| if *v > *num { *v } else { *num })
        .collect::<Array<_, _>>()
}

pub fn minimum<A, D>(
    num: &A,
    num_array: Array<A, D>,
) -> ArrayBase<ndarray::OwnedRepr<A>, Dim<[usize; 1]>>
where
    A: std::cmp::PartialOrd + std::marker::Copy,
    D: Dimension,
{
    num_array
        .iter()
        .map(|v| if *v > *num { *num } else { *v })
        .collect::<Array<_, _>>()
}

pub fn map_index(
    index: &[usize],
    arr: &ArrayBase<ndarray::ViewRepr<&f32>, ndarray::Dim<[usize; 1]>>,
) -> ArrayBase<ndarray::OwnedRepr<f32>, ndarray::Dim<[usize; 1]>> {
    index.iter().map(|v| arr[*v]).collect::<Array<_, _>>()
}

pub fn clamp<T: PartialOrd>(input: T, min: T, max: T) -> T {
    debug_assert!(min <= max, "min must be less than or equal to max");
    if input < min {
        min
    } else if input > max {
        max
    } else {
        input
    }
}

pub enum SuppressionType {
    Union,
    Min,
}

pub fn get_scales(width: u32, height: u32, minsize: u32, factor: f64) -> Vec<f64> {
    let mut m = 12.0 / minsize as f64;
    let mut minl = cmp::min(width, height) as f64 * m;

    // create scale pyramid
    let mut scales = Vec::new();
    while minl >= 12.0 {
        scales.push(m);
        m *= factor;
        minl *= factor;
    }

    // if len(scales) > self.max_n_scales:  # probably won't happen...
    //     raise ValueError('Too many scales, try increasing minsize '
    //                      'or decreasing factor.')

    scales
}

pub fn nms(boxes: &Array2<f32>, threshold: f32, s_type: SuppressionType) -> Vec<usize> {
    let areas = boxes
        .axis_iter(Axis(0))
        .map(|x| (x[2] - x[0] + 1.0) * (x[3] - x[1] + 1.0))
        .collect::<Array<_, _>>();

    let mut sorted_idx = boxes
        .slice(s![.., 4])
        .indexed_iter()
        .map(|(index, &item)| index)
        .collect::<Vec<usize>>();
    sorted_idx.sort_unstable_by(|a, b| boxes[[*a, 4]].partial_cmp(&boxes[[*b, 4]]).unwrap());

    let xx1 = boxes.slice(s![.., 0]);
    let yy1 = boxes.slice(s![.., 1]);
    let xx2 = boxes.slice(s![.., 2]);
    let yy2 = boxes.slice(s![.., 3]);

    let mut pick: Vec<usize> = vec![];
    loop {
        if sorted_idx.len() <= 0 {
            break;
        }

        let (begin, last) = sorted_idx.split_at(sorted_idx.len() - 1);

        let tx1 = maximum(&xx1[last[0]], map_index(begin, &xx1));
        let ty1 = maximum(&yy1[last[0]], map_index(begin, &yy1));
        let tx2 = minimum(&xx2[last[0]], map_index(begin, &xx2));
        let ty2 = minimum(&yy2[last[0]], map_index(begin, &yy2));

        let tw = maximum(&0.0, tx2 - tx1 + 1.0);
        let th = maximum(&0.0, ty2 - ty1 + 1.0);
        let inter = tw * th;

        let iou = match s_type {
            SuppressionType::Min => {
                inter / minimum(&areas[last[0]], map_index(begin, &areas.view()))
            }
            SuppressionType::Union => {
                inter.clone() / (areas[last[0]] + map_index(begin, &areas.view()) - inter)
            }
        };
        pick.push(last[0]);
        sorted_idx = sorted_idx
            .iter()
            .enumerate()
            .filter_map(|(index, &item)| {
                if (index < iou.len()) && (iou[index] <= threshold) {
                    Some(item)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
    }

    pick
}

pub fn clip_dets(in_dets: &Array2<f32>, img_w: u32, img_h: u32) -> Array2<f32> {
    let out_dets_t = in_dets
        .axis_iter(Axis(0))
        .map(|v| {
            [
                clamp(f32::trunc(v[0]), 0.0, (img_w - 1) as f32),
                clamp(f32::trunc(v[1]), 0.0, (img_h - 1) as f32),
                clamp(f32::trunc(v[2]), 0.0, (img_w - 1) as f32),
                clamp(f32::trunc(v[3]), 0.0, (img_h - 1) as f32),
                v[4],
            ]
        })
        .collect::<Vec<_>>();

    let out_dets = Array::from_shape_vec(
        (in_dets.dim().0, in_dets.dim().1),
        out_dets_t.iter().flatten().map(|v| *v).collect::<Vec<_>>(),
    )
    .unwrap();

    out_dets
}

pub fn rescale(image: &DynamicImage, min_size: u32) -> (RgbImage, u32) {
    let scale = f32::min(720.0 / image.height() as f32, 1280.0 / image.width() as f32);
    let (width, height) = if scale < 1.0 {
        (
            (image.width() as f32 * scale).ceil() as u32,
            (image.height() as f32 * scale).ceil() as u32,
        )
    } else {
        (image.width(), image.height())
    };
    let ms = || {
        if scale < 1.0 {
            return cmp::max((min_size as f32 * scale).ceil() as u32, 40);
        } else {
            return min_size;
        }
    };
    (
        image.resize(width, height, FilterType::Nearest).to_rgb8(),
        ms(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper::*;
    use ndarray::{Array, Array3};
    use ndarray_npy::read_npy;

    #[test]
    fn test_nms() {
        let pick_res: Vec<Vec<usize>> = vec![
            vec![36, 12, 43, 31, 22, 6, 44, 3, 19, 47, 33, 2, 0, 1, 8],
            vec![20, 25, 5, 1, 13, 6, 7],
            vec![10, 2, 15, 0, 17, 18, 3],
            vec![3, 9, 0, 10, 1, 2],
        ];

        let scales = get_scales(1076, 720, 40, 0.709);
        for (i, scale) in scales.iter().enumerate() {
            let pnetboxes: Array2<f32> =
                read_npy(format!("test_resources/pnetboxes{}.npy", i)).unwrap();
            let pick = nms(&pnetboxes, 0.5, SuppressionType::Union);
            if pick.len() > 0 {
                assert_eq!(pick, pick_res[i]);
            }
        }
    }

    #[test]
    fn test_clip_dets() {
        let indexedboxes: Array2<f32> = read_npy("test_resources/indexedboxes.npy").unwrap();
        let dts: Array2<f32> = read_npy("test_resources/dets.npy").unwrap();

        let dets = clip_dets(&indexedboxes, 1076, 720);

        assert_eq!(dts, dets);
    }

    #[test]
    fn test_rescale() {
        let img1 = image::open("test_resources/2020-11-21-144033.jpg").unwrap();

        let (scaled_image1, min_size) = rescale(&img1, 40);

        assert_eq!(min_size, 40);
        assert_eq!(scaled_image1.width(), 640);
        assert_eq!(scaled_image1.height(), 360);

        let img2 = image::open("test_resources/DSC_0003.JPG").unwrap();

        let (scaled_image2, min_size) = rescale(&img2, 40);

        assert_eq!(min_size, 40);
        assert_eq!(scaled_image2.width(), 1076);
        assert_eq!(scaled_image2.height(), 720);

        // display_image(&scaled_image1);

        // display_image(&img2);

        // display_image(&scaled_image2);
    }
}
