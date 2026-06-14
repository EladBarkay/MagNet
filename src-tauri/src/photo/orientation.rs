use crate::project::model::{Orientation, Photo};

pub fn detect_orientation(photo: &Photo) -> Orientation {
    photo.effective_orientation()
}
