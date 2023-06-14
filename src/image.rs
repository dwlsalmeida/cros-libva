// Copyright 2022 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::rc::Rc;

use crate::bindings;
use crate::picture::Picture;
use crate::picture::PictureSync;
use crate::va_check;
use crate::Display;
use crate::SurfaceMemoryDescriptor;
use crate::VaError;

/// Wrapper around `VAImage` that is tied to the lifetime of a given `Picture`.
///
/// An image is used to either get the surface data to client memory, or to copy image data in
/// client memory to a surface.
pub struct Image<'a> {
    /// The display from which the image was created, so we can unmap it upon destruction.
    display: Rc<Display>,
    /// The `VAImage` returned by libva.
    image: bindings::VAImage,
    /// The mapped surface data.
    data: &'a [u8],
    /// Whether the image was derived using the `vaDeriveImage` API or created using the
    /// `vaCreateImage` API.
    derived: bool,
}

impl<'a> Image<'a> {
    /// Helper method to map a `VAImage` using `vaMapBuffer` and return an `Image`.
    ///
    /// Returns an error if the mapping failed.
    pub(crate) fn new<D: SurfaceMemoryDescriptor>(
        picture: &'a Picture<PictureSync, D>,
        image: bindings::VAImage,
        derived: bool,
    ) -> Result<Self, VaError> {
        let mut addr = std::ptr::null_mut();

        // Safe since `picture.inner.context` represents a valid `VAContext` and `image` has been
        // successfully created at this point.
        match va_check(unsafe {
            bindings::vaMapBuffer(picture.display().handle(), image.buf, &mut addr)
        }) {
            Ok(_) => {
                // Safe since `addr` points to data mapped onto our address space since we called
                // `vaMapBuffer` above, which also guarantees that the data is valid for
                // `image.data_size`.
                let data =
                    unsafe { std::slice::from_raw_parts_mut(addr as _, image.data_size as usize) };
                Ok(Image {
                    display: Rc::clone(picture.display()),
                    image,
                    data,
                    derived,
                })
            }
            Err(e) => {
                // Safe because `picture.inner.context` represents a valid `VAContext` and `image`
                // represents a valid `VAImage`.
                unsafe {
                    bindings::vaDestroyImage(picture.display().handle(), image.image_id);
                }

                Err(e)
            }
        }
    }

    /// Get a reference to the underlying `VAImage` that describes this image.
    pub fn image(&self) -> &bindings::VAImage {
        &self.image
    }

    /// Returns whether this image is directly derived from its underlying `Picture`, as opposed to
    /// being a view/copy of said `Picture` in a guaranteed pixel format.
    pub fn is_derived(&self) -> bool {
        self.derived
    }
}

impl<'a> AsRef<[u8]> for Image<'a> {
    fn as_ref(&self) -> &[u8] {
        self.data
    }
}

impl<'a> Drop for Image<'a> {
    fn drop(&mut self) {
        unsafe {
            // Safe since the buffer is mapped in `Image::new`, so `self.image.buf` points to a
            // valid `VABufferID`.
            bindings::vaUnmapBuffer(self.display.handle(), self.image.buf);
            // Safe since `self.image` represents a valid `VAImage`.
            bindings::vaDestroyImage(self.display.handle(), self.image.image_id);
        }
    }
}
