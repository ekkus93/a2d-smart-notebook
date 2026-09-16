use a2d_domain::A2dError;
use a2d_layout::{MarkerRole, PageLayout};

use crate::{
    detection::{ImagePoint, ResolvedPageMarkers},
    encoded::{OwnedGrayImage, OwnedRgbImage},
    error::{processing_error, validation_error},
    input::{GrayFrame, ImageRotation},
};

const GEOMETRY_EPSILON: f64 = 1.0e-9;
const MIN_PIVOT_RATIO: f64 = 1.0e-12;
const BOUNDS_EPSILON: f64 = 1.0e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageQuad {
    pub top_left: ImagePoint,
    pub top_right: ImagePoint,
    pub bottom_right: ImagePoint,
    pub bottom_left: ImagePoint,
}

impl ImageQuad {
    pub const fn new(
        top_left: ImagePoint,
        top_right: ImagePoint,
        bottom_right: ImagePoint,
        bottom_left: ImagePoint,
    ) -> Self {
        Self { top_left, top_right, bottom_right, bottom_left }
    }

    pub const fn points(self) -> [ImagePoint; 4] {
        [self.top_left, self.top_right, self.bottom_right, self.bottom_left]
    }

    pub fn signed_area(self) -> f64 {
        let points = self.points();
        let mut twice_area = 0.0;
        for index in 0..4 {
            let current = points[index];
            let next = points[(index + 1) % 4];
            twice_area += current.x * next.y - current.y * next.x;
        }
        twice_area * 0.5
    }

    pub fn validate(self, label: &'static str) -> Result<(), A2dError> {
        let points = self.points();
        if points.iter().any(|point| !point.x.is_finite() || !point.y.is_finite()) {
            return Err(validation_error("HOMOGRAPHY_QUAD_NON_FINITE", format!("{label} quadrilateral contains a non-finite point")));
        }
        for index in 0..4 {
            let edge_length_squared = distance_squared(points[index], points[(index + 1) % 4]);
            if edge_length_squared <= GEOMETRY_EPSILON * GEOMETRY_EPSILON {
                return Err(validation_error("HOMOGRAPHY_QUAD_DEGENERATE", format!("{label} quadrilateral has a zero-length edge")));
            }
        }
        if segments_intersect(points[0], points[1], points[2], points[3]) || segments_intersect(points[1], points[2], points[3], points[0]) {
            return Err(validation_error("HOMOGRAPHY_QUAD_SELF_INTERSECTING", format!("{label} quadrilateral is self-intersecting")));
        }
        let mut expected_sign = 0.0;
        for index in 0..4 {
            let cross = cross_product(points[index], points[(index + 1) % 4], points[(index + 2) % 4]);
            if cross.abs() <= GEOMETRY_EPSILON {
                return Err(validation_error("HOMOGRAPHY_QUAD_DEGENERATE", format!("{label} quadrilateral has collinear adjacent edges")));
            }
            if expected_sign == 0.0 { expected_sign = cross.signum(); } else if cross.signum() != expected_sign {
                return Err(validation_error("HOMOGRAPHY_QUAD_NON_CONVEX", format!("{label} quadrilateral is not convex")));
            }
        }
        if self.signed_area().abs() <= GEOMETRY_EPSILON {
            return Err(validation_error("HOMOGRAPHY_QUAD_DEGENERATE", format!("{label} quadrilateral has negligible area")));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RectificationLimits { max_output_pixels: u64, max_output_bytes: u64 }
impl RectificationLimits {
    pub fn new(max_output_pixels: u64, max_output_bytes: u64) -> Result<Self, A2dError> {
        if max_output_pixels == 0 { return Err(validation_error("RECTIFICATION_PIXEL_LIMIT_INVALID", "maximum rectified pixel count must be greater than zero")); }
        if max_output_bytes == 0 { return Err(validation_error("RECTIFICATION_BYTE_LIMIT_INVALID", "maximum rectified byte count must be greater than zero")); }
        Ok(Self { max_output_pixels, max_output_bytes })
    }
    pub const fn max_output_pixels(self) -> u64 { self.max_output_pixels }
    pub const fn max_output_bytes(self) -> u64 { self.max_output_bytes }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RectifiedImageSize { width: u32, height: u32, pixel_count: u64, rgb_byte_count: u64 }
impl RectifiedImageSize {
    pub fn new(width: u32, height: u32, limits: RectificationLimits) -> Result<Self, A2dError> {
        if width < 2 || height < 2 { return Err(validation_error("RECTIFICATION_DIMENSIONS_INVALID", format!("rectified dimensions must be at least 2x2, got {width}x{height}"))); }
        let pixel_count = u64::from(width).checked_mul(u64::from(height)).ok_or_else(|| validation_error("RECTIFICATION_PIXEL_COUNT_OVERFLOW", format!("rectified pixel count overflow for {width}x{height}")))?;
        if pixel_count > limits.max_output_pixels() { return Err(validation_error("RECTIFICATION_PIXEL_LIMIT_EXCEEDED", format!("rectified output has {pixel_count} pixels, limit is {}", limits.max_output_pixels()))); }
        let rgb_byte_count = pixel_count.checked_mul(3).ok_or_else(|| validation_error("RECTIFICATION_BYTE_COUNT_OVERFLOW", format!("rectified RGB byte count overflow for {width}x{height}")))?;
        if rgb_byte_count > limits.max_output_bytes() { return Err(validation_error("RECTIFICATION_BYTE_LIMIT_EXCEEDED", format!("rectified RGB output requires {rgb_byte_count} bytes, limit is {}", limits.max_output_bytes()))); }
        usize::try_from(rgb_byte_count).map_err(|_| validation_error("RECTIFICATION_OUTPUT_UNSUPPORTED", "rectified output does not fit this platform's address space"))?;
        Ok(Self { width, height, pixel_count, rgb_byte_count })
    }
    pub const fn width(self) -> u32 { self.width }
    pub const fn height(self) -> u32 { self.height }
    pub const fn pixel_count(self) -> u64 { self.pixel_count }
    pub const fn rgb_byte_count(self) -> u64 { self.rgb_byte_count }
    fn destination_quad(self) -> ImageQuad {
        let max_x = f64::from(self.width - 1); let max_y = f64::from(self.height - 1);
        ImageQuad::new(ImagePoint{x:0.0,y:0.0}, ImagePoint{x:max_x,y:0.0}, ImagePoint{x:max_x,y:max_y}, ImagePoint{x:0.0,y:max_y})
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectiveTransform { source_to_destination: [[f64;3];3], destination_to_source: [[f64;3];3], pivot_ratio: f64 }
impl ProjectiveTransform {
    pub fn from_quads(source: ImageQuad, destination: ImageQuad) -> Result<Self,A2dError> {
        source.validate("source")?; destination.validate("destination")?;
        let sn=normalize_points(source.points())?; let dn=normalize_points(destination.points())?;
        let (normalized,pivot_ratio)=solve_homography(sn.points,dn.points)?;
        if !pivot_ratio.is_finite() || pivot_ratio < MIN_PIVOT_RATIO { return Err(processing_error("HOMOGRAPHY_ILL_CONDITIONED",format!("homography solve pivot ratio {pivot_ratio:e} is below {MIN_PIVOT_RATIO:e}"),true)); }
        let denormalized=multiply_3x3(dn.inverse,multiply_3x3(normalized,sn.matrix));
        let source_to_destination=normalize_matrix_scale(denormalized)?; let destination_to_source=invert_3x3(source_to_destination)?;
        let transform=Self{source_to_destination,destination_to_source,pivot_ratio};
        for (s,d) in source.points().into_iter().zip(destination.points()) { let mapped=transform.map_source_to_destination(s)?; if distance_squared(mapped,d)>1.0e-10 { return Err(processing_error("HOMOGRAPHY_CORRESPONDENCE_MISMATCH","solved homography does not reproduce its input correspondences",false)); } }
        Ok(transform)
    }
    pub const fn source_to_destination_matrix(self)->[[f64;3];3]{self.source_to_destination}
    pub const fn destination_to_source_matrix(self)->[[f64;3];3]{self.destination_to_source}
    pub const fn pivot_ratio(self)->f64{self.pivot_ratio}
    pub fn map_source_to_destination(self,point:ImagePoint)->Result<ImagePoint,A2dError>{project(self.source_to_destination,point)}
    pub fn map_destination_to_source(self,point:ImagePoint)->Result<ImagePoint,A2dError>{project(self.destination_to_source,point)}
}

#[derive(Clone, Debug, PartialEq)]
pub struct RectificationPlan { source_width:u32, source_height:u32, output_size:RectifiedImageSize, source_page_corners:ImageQuad, destination_page_corners:ImageQuad, source_marker_centers:Option<ImageQuad>, destination_marker_centers:Option<ImageQuad>, transform:ProjectiveTransform }
impl RectificationPlan {
    pub fn from_page_corners(source_width:u32,source_height:u32,source_page_corners:ImageQuad,output_size:RectifiedImageSize)->Result<Self,A2dError>{
        validate_source_dimensions(source_width,source_height)?; validate_quad_within_image(source_page_corners,source_width,source_height,"source page corners")?;
        let destination_page_corners=output_size.destination_quad(); let transform=ProjectiveTransform::from_quads(source_page_corners,destination_page_corners)?;
        Ok(Self{source_width,source_height,output_size,source_page_corners,destination_page_corners,source_marker_centers:None,destination_marker_centers:None,transform})
    }
    pub fn from_page_markers(source_width:u32,source_height:u32,markers:&ResolvedPageMarkers,layout:&PageLayout,output_size:RectifiedImageSize)->Result<Self,A2dError>{
        validate_source_dimensions(source_width,source_height)?; layout.validate()?; validate_physical_page(layout)?;
        let source_marker_centers=ImageQuad::new(markers.marker(MarkerRole::TopLeft).center,markers.marker(MarkerRole::TopRight).center,markers.marker(MarkerRole::BottomRight).center,markers.marker(MarkerRole::BottomLeft).center);
        validate_quad_within_image(source_marker_centers,source_width,source_height,"source marker centers")?;
        let destination_marker_centers=marker_destination_quad(layout,output_size)?; let transform=ProjectiveTransform::from_quads(source_marker_centers,destination_marker_centers)?;
        let destination_page_corners=output_size.destination_quad(); let source_points=destination_page_corners.points().map(|p|transform.map_destination_to_source(p)).into_iter().collect::<Result<Vec<_>,_>>()?;
        let source_page_corners=ImageQuad::new(source_points[0],source_points[1],source_points[2],source_points[3]);
        validate_quad_within_image(source_page_corners,source_width,source_height,"extrapolated source page corners")?;
        Ok(Self{source_width,source_height,output_size,source_page_corners,destination_page_corners,source_marker_centers:Some(source_marker_centers),destination_marker_centers:Some(destination_marker_centers),transform})
    }
    pub const fn source_width(&self)->u32{self.source_width} pub const fn source_height(&self)->u32{self.source_height} pub const fn output_size(&self)->RectifiedImageSize{self.output_size}
    pub const fn source_page_corners(&self)->ImageQuad{self.source_page_corners} pub const fn destination_page_corners(&self)->ImageQuad{self.destination_page_corners}
    pub const fn source_marker_centers(&self)->Option<ImageQuad>{self.source_marker_centers} pub const fn destination_marker_centers(&self)->Option<ImageQuad>{self.destination_marker_centers} pub const fn transform(&self)->ProjectiveTransform{self.transform}
    pub fn rectify_gray8(&self,source:GrayFrame<'_>)->Result<OwnedGrayImage,A2dError>{ self.validate_source_match(source.width(),source.height())?; let output_len=usize::try_from(self.output_size.pixel_count()).map_err(|_|validation_error("RECTIFICATION_OUTPUT_UNSUPPORTED","rectified Gray8 output does not fit this platform's address space"))?; let mut output=Vec::with_capacity(output_len); for y in 0..self.output_size.height(){for x in 0..self.output_size.width(){let p=self.transform.map_destination_to_source(ImagePoint{x:f64::from(x),y:f64::from(y)})?; output.push(sample_gray8(source,p)?);}} OwnedGrayImage::from_tight(self.output_size.width(),self.output_size.height(),ImageRotation::Degrees0,output)}
    pub fn rectify_rgb8(&self,source:&OwnedRgbImage)->Result<OwnedRgbImage,A2dError>{ self.validate_source_match(source.width(),source.height())?; let output_len=usize::try_from(self.output_size.rgb_byte_count()).map_err(|_|validation_error("RECTIFICATION_OUTPUT_UNSUPPORTED","rectified RGB8 output does not fit this platform's address space"))?; let mut output=Vec::with_capacity(output_len); for y in 0..self.output_size.height(){for x in 0..self.output_size.width(){let p=self.transform.map_destination_to_source(ImagePoint{x:f64::from(x),y:f64::from(y)})?; output.extend_from_slice(&sample_rgb8(source,p)?);}} OwnedRgbImage::from_tight(self.output_size.width(),self.output_size.height(),ImageRotation::Degrees0,output)}
    fn validate_source_match(&self,width:u32,height:u32)->Result<(),A2dError>{if width!=self.source_width||height!=self.source_height{return Err(validation_error("RECTIFICATION_SOURCE_DIMENSIONS_MISMATCH",format!("rectification plan expects {}x{} source pixels, got {width}x{height}",self.source_width,self.source_height)));}Ok(())}
}

#[derive(Clone,Copy)] struct NormalizedPoints{points:[ImagePoint;4],matrix:[[f64;3];3],inverse:[[f64;3];3]}
fn normalize_points(points:[ImagePoint;4])->Result<NormalizedPoints,A2dError>{let centroid=ImagePoint{x:points.iter().map(|p|p.x).sum::<f64>()/4.0,y:points.iter().map(|p|p.y).sum::<f64>()/4.0};let mean_distance=points.iter().map(|p|distance_squared(*p,centroid).sqrt()).sum::<f64>()/4.0;if !mean_distance.is_finite()||mean_distance<=GEOMETRY_EPSILON{return Err(processing_error("HOMOGRAPHY_NORMALIZATION_FAILED","quadrilateral points cannot be normalized",true));}let scale=2.0_f64.sqrt()/mean_distance;let matrix=[[scale,0.0,-scale*centroid.x],[0.0,scale,-scale*centroid.y],[0.0,0.0,1.0]];let inverse=[[1.0/scale,0.0,centroid.x],[0.0,1.0/scale,centroid.y],[0.0,0.0,1.0]];let normalized=points.map(|p|ImagePoint{x:scale*(p.x-centroid.x),y:scale*(p.y-centroid.y)});Ok(NormalizedPoints{points:normalized,matrix,inverse})}
fn solve_homography(source:[ImagePoint;4],destination:[ImagePoint;4])->Result<([[f64;3];3],f64),A2dError>{let mut a=[[0.0_f64;9];8];for i in 0..4{let s=source[i];let d=destination[i];let r=i*2;a[r]=[s.x,s.y,1.0,0.0,0.0,0.0,-d.x*s.x,-d.x*s.y,d.x];a[r+1]=[0.0,0.0,0.0,s.x,s.y,1.0,-d.y*s.x,-d.y*s.y,d.y];}let mut min=f64::INFINITY;let mut max: f64=0.0;for c in 0..8{let pr=(c..8).max_by(|l,r|a[*l][c].abs().total_cmp(&a[*r][c].abs())).unwrap();let p=a[pr][c].abs();if !p.is_finite()||p<=GEOMETRY_EPSILON{return Err(processing_error("HOMOGRAPHY_SOLVE_SINGULAR",format!("homography solve has a singular pivot in column {c}"),true));}min=min.min(p);max=max.max(p);a.swap(c,pr);let div=a[c][c];for v in &mut a[c][c..]{*v/=div;}let pv=a[c];for (r,row) in a.iter_mut().enumerate(){if r==c{continue}let f=row[c];if f==0.0{continue}for(t,p)in row[c..].iter_mut().zip(&pv[c..]){*t-=f*p;}}}let s=a.map(|r|r[8]);if s.iter().any(|v|!v.is_finite()){return Err(processing_error("HOMOGRAPHY_SOLVE_NON_FINITE","homography solve produced a non-finite coefficient",false));}Ok(([[s[0],s[1],s[2]],[s[3],s[4],s[5]],[s[6],s[7],1.0]],min/max))}
fn normalize_matrix_scale(mut m:[[f64;3];3])->Result<[[f64;3];3],A2dError>{let max=m.iter().flat_map(|r|r.iter()).map(|v|v.abs()).fold(0.0_f64,f64::max);if !max.is_finite()||max<=GEOMETRY_EPSILON{return Err(processing_error("HOMOGRAPHY_MATRIX_INVALID","homography matrix has no finite non-zero scale",false));}for r in &mut m{for v in r{*v/=max;}}Ok(m)}
fn invert_3x3(m:[[f64;3];3])->Result<[[f64;3];3],A2dError>{let d=m[0][0]*(m[1][1]*m[2][2]-m[1][2]*m[2][1])-m[0][1]*(m[1][0]*m[2][2]-m[1][2]*m[2][0])+m[0][2]*(m[1][0]*m[2][1]-m[1][1]*m[2][0]);if !d.is_finite()||d.abs()<=GEOMETRY_EPSILON{return Err(processing_error("HOMOGRAPHY_MATRIX_SINGULAR","homography matrix is singular",true));}let i=1.0/d;let inv=[[(m[1][1]*m[2][2]-m[1][2]*m[2][1])*i,(m[0][2]*m[2][1]-m[0][1]*m[2][2])*i,(m[0][1]*m[1][2]-m[0][2]*m[1][1])*i],[(m[1][2]*m[2][0]-m[1][0]*m[2][2])*i,(m[0][0]*m[2][2]-m[0][2]*m[2][0])*i,(m[0][2]*m[1][0]-m[0][0]*m[1][2])*i],[(m[1][0]*m[2][1]-m[1][1]*m[2][0])*i,(m[0][1]*m[2][0]-m[0][0]*m[2][1])*i,(m[0][0]*m[1][1]-m[0][1]*m[1][0])*i]];if inv.iter().flat_map(|r|r.iter()).any(|v|!v.is_finite()){return Err(processing_error("HOMOGRAPHY_MATRIX_NON_FINITE","inverse homography contains a non-finite coefficient",false));}Ok(inv)}
fn multiply_3x3(l:[[f64;3];3],r:[[f64;3];3])->[[f64;3];3]{let mut o=[[0.0;3];3];for row in 0..3{for col in 0..3{o[row][col]=(0..3).map(|i|l[row][i]*r[i][col]).sum();}}o}
fn project(m:[[f64;3];3],p:ImagePoint)->Result<ImagePoint,A2dError>{if !p.x.is_finite()||!p.y.is_finite(){return Err(validation_error("HOMOGRAPHY_POINT_NON_FINITE","projective transform input point is non-finite"));}let d=m[2][0]*p.x+m[2][1]*p.y+m[2][2];if !d.is_finite()||d.abs()<=GEOMETRY_EPSILON{return Err(processing_error("HOMOGRAPHY_PROJECTION_INVALID","projective transform maps a point to infinity",true));}let x=(m[0][0]*p.x+m[0][1]*p.y+m[0][2])/d;let y=(m[1][0]*p.x+m[1][1]*p.y+m[1][2])/d;if !x.is_finite()||!y.is_finite(){return Err(processing_error("HOMOGRAPHY_PROJECTION_NON_FINITE","projective transform produced a non-finite point",false));}Ok(ImagePoint{x,y})}
fn marker_destination_quad(layout:&PageLayout,output_size:RectifiedImageSize)->Result<ImageQuad,A2dError>{let point_for=|role:MarkerRole|->Result<ImagePoint,A2dError>{let marker=layout.markers.iter().find(|m|m.role==role).ok_or_else(||validation_error("RECTIFICATION_LAYOUT_MARKER_MISSING",format!("layout is missing {} marker",role.as_id_str())))?;let x=marker.rect.origin.x_mm+marker.rect.size.width_mm*0.5;let y=marker.rect.origin.y_mm+marker.rect.size.height_mm*0.5;if !x.is_finite()||!y.is_finite(){return Err(validation_error("RECTIFICATION_LAYOUT_GEOMETRY_INVALID",format!("layout {} marker center is non-finite",role.as_id_str())));}Ok(ImagePoint{x:x/layout.physical_size.width_mm*f64::from(output_size.width()-1),y:y/layout.physical_size.height_mm*f64::from(output_size.height()-1)})};Ok(ImageQuad::new(point_for(MarkerRole::TopLeft)?,point_for(MarkerRole::TopRight)?,point_for(MarkerRole::BottomRight)?,point_for(MarkerRole::BottomLeft)?))}
fn validate_physical_page(layout:&PageLayout)->Result<(),A2dError>{if !layout.physical_size.width_mm.is_finite()||!layout.physical_size.height_mm.is_finite()||layout.physical_size.width_mm<=0.0||layout.physical_size.height_mm<=0.0{return Err(validation_error("RECTIFICATION_LAYOUT_SIZE_INVALID",format!("layout physical size must be finite and positive, got {}x{}mm",layout.physical_size.width_mm,layout.physical_size.height_mm)));}Ok(())}
fn validate_source_dimensions(width:u32,height:u32)->Result<(),A2dError>{if width<2||height<2{return Err(validation_error("RECTIFICATION_SOURCE_DIMENSIONS_INVALID",format!("source dimensions must be at least 2x2, got {width}x{height}")));}Ok(())}
fn validate_quad_within_image(quad:ImageQuad,width:u32,height:u32,label:&'static str)->Result<(),A2dError>{quad.validate(label)?;let max_x=f64::from(width-1);let max_y=f64::from(height-1);if quad.points().iter().any(|p|p.x < -BOUNDS_EPSILON||p.y < -BOUNDS_EPSILON||p.x>max_x+BOUNDS_EPSILON||p.y>max_y+BOUNDS_EPSILON){return Err(validation_error("RECTIFICATION_SOURCE_CORNERS_OUT_OF_BOUNDS",format!("{label} extend outside the {width}x{height} source image")));}Ok(())}
fn sample_gray8(source:GrayFrame<'_>,p:ImagePoint)->Result<u8,A2dError>{let(x0,y0,x1,y1,xf,yf)=sampling_coordinates(p,source.width(),source.height())?;let b=source.bytes();let s=source.row_stride();Ok(bilinear(f64::from(b[y0*s+x0]),f64::from(b[y0*s+x1]),f64::from(b[y1*s+x0]),f64::from(b[y1*s+x1]),xf,yf).round().clamp(0.0,255.0)as u8)}
fn sample_rgb8(source:&OwnedRgbImage,p:ImagePoint)->Result<[u8;3],A2dError>{let(x0,y0,x1,y1,xf,yf)=sampling_coordinates(p,source.width(),source.height())?;let b=source.bytes();let s=source.row_stride();let pixel=|x:usize,y:usize,c:usize|f64::from(b[y*s+x*3+c]);let mut o=[0u8;3];for(c,out)in o.iter_mut().enumerate(){*out=bilinear(pixel(x0,y0,c),pixel(x1,y0,c),pixel(x0,y1,c),pixel(x1,y1,c),xf,yf).round().clamp(0.0,255.0)as u8;}Ok(o)}
fn sampling_coordinates(p:ImagePoint,width:u32,height:u32)->Result<(usize,usize,usize,usize,f64,f64),A2dError>{let max_x=f64::from(width-1);let max_y=f64::from(height-1);if !p.x.is_finite()||!p.y.is_finite()||p.x < -BOUNDS_EPSILON||p.y < -BOUNDS_EPSILON||p.x>max_x+BOUNDS_EPSILON||p.y>max_y+BOUNDS_EPSILON{return Err(processing_error("RECTIFICATION_SAMPLE_OUT_OF_BOUNDS",format!("rectification sample ({}, {}) is outside {width}x{height} source bounds",p.x,p.y),true));}let x=p.x.clamp(0.0,max_x);let y=p.y.clamp(0.0,max_y);let x0=x.floor()as usize;let y0=y.floor()as usize;let x1=(x0+1).min(width as usize-1);let y1=(y0+1).min(height as usize-1);Ok((x0,y0,x1,y1,x-x0 as f64,y-y0 as f64))}
fn bilinear(tl:f64,tr:f64,bl:f64,br:f64,xf:f64,yf:f64)->f64{let top=tl+(tr-tl)*xf;let bottom=bl+(br-bl)*xf;top+(bottom-top)*yf}
fn cross_product(o:ImagePoint,a:ImagePoint,b:ImagePoint)->f64{(a.x-o.x)*(b.y-o.y)-(a.y-o.y)*(b.x-o.x)}
fn distance_squared(a:ImagePoint,b:ImagePoint)->f64{let dx=a.x-b.x;let dy=a.y-b.y;dx*dx+dy*dy}
fn segments_intersect(a:ImagePoint,b:ImagePoint,c:ImagePoint,d:ImagePoint)->bool{let fa=cross_product(a,b,c);let fb=cross_product(a,b,d);let sa=cross_product(c,d,a);let sb=cross_product(c,d,b);fa*fb < -GEOMETRY_EPSILON && sa*sb < -GEOMETRY_EPSILON}

#[cfg(test)]
mod tests {
    use a2d_domain::LayoutId;
    use a2d_layout::{CalibrationMark, ContentStyle, MarkerPlacement, geometry::{PhysicalPoint,PhysicalRect,PhysicalSize}};
    use crate::{MarkerDetection,MarkerFamily,PageOrientation,ResolvedMarker,input::ImageLimits};
    use super::*;
    fn limits()->RectificationLimits{RectificationLimits::new(1_000_000,3_000_000).unwrap()}
    fn point(x:f64,y:f64)->ImagePoint{ImagePoint{x,y}}
    fn full_quad(width:u32,height:u32)->ImageQuad{ImageQuad::new(point(0.0,0.0),point(f64::from(width-1),0.0),point(f64::from(width-1),f64::from(height-1)),point(0.0,f64::from(height-1)))}
    #[test] fn identity_homography_preserves_points(){let q=full_quad(10,20);let t=ProjectiveTransform::from_quads(q,q).unwrap();for c in [point(0.0,0.0),point(3.25,7.5),point(9.0,19.0)]{let m=t.map_source_to_destination(c).unwrap();assert!((m.x-c.x).abs()<1e-9);assert!((m.y-c.y).abs()<1e-9);}}
    #[test] fn perspective_homography_reproduces_all_correspondences(){let s=ImageQuad::new(point(12.0,8.0),point(90.0,4.0),point(96.0,110.0),point(5.0,100.0));let d=full_quad(80,100);let t=ProjectiveTransform::from_quads(s,d).unwrap();for(s,d)in s.points().into_iter().zip(d.points()){let m=t.map_source_to_destination(s).unwrap();assert!((m.x-d.x).abs()<1e-7);assert!((m.y-d.y).abs()<1e-7);}}
    #[test] fn rejects_self_intersecting_and_concave_quadrilaterals(){let b=ImageQuad::new(point(0.0,0.0),point(10.0,10.0),point(10.0,0.0),point(0.0,10.0));assert_eq!(ProjectiveTransform::from_quads(b,full_quad(10,10)).unwrap_err().code.to_string(),"HOMOGRAPHY_QUAD_SELF_INTERSECTING");let c=ImageQuad::new(point(0.0,0.0),point(10.0,0.0),point(4.0,4.0),point(0.0,10.0));assert_eq!(ProjectiveTransform::from_quads(c,full_quad(10,10)).unwrap_err().code.to_string(),"HOMOGRAPHY_QUAD_NON_CONVEX");}
    #[test] fn rejects_degenerate_quadrilateral(){let d=ImageQuad::new(point(0.0,0.0),point(10.0,0.0),point(20.0,0.0),point(0.0,10.0));assert_eq!(ProjectiveTransform::from_quads(d,full_quad(10,10)).unwrap_err().code.to_string(),"HOMOGRAPHY_QUAD_DEGENERATE");}
    #[test] fn identity_gray_warp_matches_reference_bytes(){let bytes:Vec<u8>=(0..16).collect();let frame=GrayFrame::new(4,4,4,ImageRotation::Degrees270,&bytes,ImageLimits::new(16).unwrap()).unwrap();let p=RectificationPlan::from_page_corners(4,4,full_quad(4,4),RectifiedImageSize::new(4,4,limits()).unwrap()).unwrap();let o=p.rectify_gray8(frame).unwrap();assert_eq!(o.bytes(),bytes);assert_eq!(o.rotation(),ImageRotation::Degrees0);}
    #[test] fn identity_rgb_warp_matches_reference_bytes(){let bytes:Vec<u8>=(0..48).collect();let source=OwnedRgbImage::from_tight(4,4,ImageRotation::Degrees180,bytes.clone()).unwrap();let p=RectificationPlan::from_page_corners(4,4,full_quad(4,4),RectifiedImageSize::new(4,4,limits()).unwrap()).unwrap();let o=p.rectify_rgb8(&source).unwrap();assert_eq!(o.bytes(),bytes);assert_eq!(o.rotation(),ImageRotation::Degrees0);}
    #[test] fn rejects_source_page_corners_outside_the_image(){let s=ImageQuad::new(point(-1.0,0.0),point(9.0,0.0),point(9.0,9.0),point(0.0,9.0));assert_eq!(RectificationPlan::from_page_corners(10,10,s,RectifiedImageSize::new(10,10,limits()).unwrap()).unwrap_err().code.to_string(),"RECTIFICATION_SOURCE_CORNERS_OUT_OF_BOUNDS");}
    #[test] fn rejects_source_dimensions_that_do_not_match_the_plan(){let bytes=[0u8;25];let frame=GrayFrame::new(5,5,5,ImageRotation::Degrees0,&bytes,ImageLimits::new(25).unwrap()).unwrap();let p=RectificationPlan::from_page_corners(4,4,full_quad(4,4),RectifiedImageSize::new(4,4,limits()).unwrap()).unwrap();assert_eq!(p.rectify_gray8(frame).unwrap_err().code.to_string(),"RECTIFICATION_SOURCE_DIMENSIONS_MISMATCH");}
    #[test] fn enforces_output_memory_limits(){assert_eq!(RectifiedImageSize::new(100,100,RectificationLimits::new(10_000,29_999).unwrap()).unwrap_err().code.to_string(),"RECTIFICATION_BYTE_LIMIT_EXCEEDED");}
    fn layout()->PageLayout{let marker=|role,x,y|MarkerPlacement{role,rect:PhysicalRect::new(x,y,10.0,10.0)};PageLayout{id:LayoutId::parse("RECTIFY-TEST").unwrap(),physical_size:PhysicalSize::new(100.0,200.0),safe_margin_mm:0.0,quiet_zone_mm:1.0,content_rect:PhysicalRect::new(20.0,20.0,60.0,140.0),markers:[marker(MarkerRole::TopLeft,5.0,5.0),marker(MarkerRole::TopRight,85.0,5.0),marker(MarkerRole::BottomLeft,5.0,185.0),marker(MarkerRole::BottomRight,85.0,185.0)],qr_rect:PhysicalRect::new(42.5,170.0,15.0,15.0),visible_page_number_rect:None,calibration:CalibrationMark{rect:PhysicalRect{origin:PhysicalPoint::new(40.0,2.0),size:PhysicalSize::new(20.0,1.0)},reference_length_mm:20.0},content_style:ContentStyle::Blank}}
    fn detection(id:u32,center:ImagePoint)->MarkerDetection{MarkerDetection{family:MarkerFamily::TagStandard41h12,id,hamming_errors:0,decision_margin:100.0,center,corners:[center;4]}}
    #[test] fn page_marker_plan_preserves_correspondence_and_source_page_corners(){let r=ResolvedPageMarkers{markers:[ResolvedMarker{role:MarkerRole::TopLeft,detection:detection(1,point(20.0,20.0))},ResolvedMarker{role:MarkerRole::TopRight,detection:detection(2,point(180.0,20.0))},ResolvedMarker{role:MarkerRole::BottomLeft,detection:detection(3,point(20.0,380.0))},ResolvedMarker{role:MarkerRole::BottomRight,detection:detection(4,point(180.0,380.0))}],orientation:PageOrientation::Degrees0,unexpected_tag_ids:Vec::new()};let p=RectificationPlan::from_page_markers(201,401,&r,&layout(),RectifiedImageSize::new(100,200,limits()).unwrap()).unwrap();for(s,e)in p.source_marker_centers().unwrap().points().into_iter().zip(p.destination_marker_centers().unwrap().points()){let a=p.transform().map_source_to_destination(s).unwrap();assert!((a.x-e.x).abs()<1e-7);assert!((a.y-e.y).abs()<1e-7);}let s=p.source_page_corners();assert!((s.top_left.x).abs()<1e-7);assert!((s.top_left.y).abs()<1e-7);assert!((s.bottom_right.x-200.0).abs()<1e-7);assert!((s.bottom_right.y-400.0).abs()<1e-7);}
}
