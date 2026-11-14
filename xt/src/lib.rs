use std::cell::RefCell;
use cad_ir::brep::{Curve, Surface};

// Full circular inner loops on planar faces are candidate holes. Partial
// arcs and non-uniformly scaled instances are deliberately not labelled.
fn circles(scene: &cad_ir::scene::Scene) -> Vec<serde_json::Value> {
    let mut result = Vec::new();
    for inst in scene.instances() {
        let Some(solid) = &scene.geometry_of(inst.geometry).brep else { continue };
        for face in &solid.faces {
            if !matches!(solid.surfaces[face.surface.index()], Surface::Plane { .. }) { continue; }
            for bound in face.bounds.iter().filter(|b| !b.outer && b.halves.len() == 1) {
                let edge = &solid.edges[bound.halves[0].edge.index()];
                let Curve::Circle { frame, radius } = &solid.curves[edge.curve.index()] else { continue };
                if edge.range.span().abs() < std::f64::consts::TAU - 1e-4 { continue; }
                let sx = inst.transform.direction(frame.ref_dir).length();
                let sy = inst.transform.direction(frame.y_dir()).length();
                if sx <= 0.0 || ((sx - sy) / sx).abs() > 1e-5 { continue; }
                let center = inst.transform.point(frame.origin);
                let axis = inst.transform.direction(frame.axis).normalized_or(frame.axis);
                result.push(serde_json::json!({"center":[center.x,center.y,center.z],"axis":[axis.x,axis.y,axis.z],"radius":radius * sx}));
                if result.len() >= 300 { return result; }
            }
        }
    }
    result
}

thread_local! {
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static INFO: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn xt_alloc(len: usize) -> *mut u8 {
    let data = vec![0u8; len].into_boxed_slice();
    Box::into_raw(data) as *mut u8
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn xt_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len))) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn xt_convert(ptr: *const u8, len: usize) -> u32 {
    OUTPUT.with(|b| b.borrow_mut().clear());
    let convert = || -> Result<(Vec<u8>, serde_json::Value), String> {
        if ptr.is_null() || len == 0 { return Err("文件为空".into()); }
        let input = unsafe { std::slice::from_raw_parts(ptr, len) };
        let text = xt_parser::decode(input.to_vec());
        let (mut scene, read_report) = cad_xt::to_scene_owned(text, &cad_xt::LowerOptions::default())
            .map_err(|e| format!("无法解析 X_T：{e}"))?;
        if scene.geometry.is_empty() { return Err("文件中没有可显示的几何体".into()); }
        let holes = circles(&scene);
        let tess_report = cad_tess::tessellate_scene(&mut scene, &cad_tess::Options::default());
        if tess_report.triangles == 0 { return Err("未能生成可显示的三角网格".into()); }
        let options = cad_export::Options { up_axis: cad_export::UpAxis::Z, ..Default::default() };
        let glb = cad_export::glb::write_bytes(&scene, &options).map_err(|e| format!("生成预览失败：{e}"))?;
        let info = serde_json::json!({
            "triangles": tess_report.triangles,
            "faces": tess_report.faces_ok,
            "failed_faces": tess_report.failed.len(),
            "skipped": read_report.skipped.len(),
            "truncated": read_report.truncated,
            "holes": holes,
        });
        Ok((glb, info))
    };
    match convert() {
        Ok((data, info)) => {
            OUTPUT.with(|b| *b.borrow_mut() = data);
            INFO.with(|b| *b.borrow_mut() = info.to_string().into_bytes());
            0
        }
        Err(error) => {
            INFO.with(|b| *b.borrow_mut() = error.into_bytes());
            1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xt_output_ptr() -> *const u8 { OUTPUT.with(|b| b.borrow().as_ptr()) }
#[unsafe(no_mangle)]
pub extern "C" fn xt_output_len() -> usize { OUTPUT.with(|b| b.borrow().len()) }
#[unsafe(no_mangle)]
pub extern "C" fn xt_info_ptr() -> *const u8 { INFO.with(|b| b.borrow().as_ptr()) }
#[unsafe(no_mangle)]
pub extern "C" fn xt_info_len() -> usize { INFO.with(|b| b.borrow().len()) }

