use std::sync::atomic::Ordering;

use crate::{utils::Serial, wayland::shell::xdg::XdgPositionerUserData};

use wayland_protocols::xdg::shell::server::xdg_popup::{self, XdgPopup};

use wayland_server::{
    backend::{ClientId, ObjectId},
    DataInit, Dispatch, DisplayHandle, Resource,
};

use super::{PopupConfigure, XdgShellHandler, XdgShellState, XdgShellSurfaceUserData, XdgSurfaceUserData};

impl<D> Dispatch<XdgPopup, XdgShellSurfaceUserData, D> for XdgShellState
where
    D: Dispatch<XdgPopup, XdgShellSurfaceUserData>,
    D: XdgShellHandler,
    D: 'static,
{
    fn request(
        state: &mut D,
        _client: &wayland_server::Client,
        popup: &XdgPopup,
        request: xdg_popup::Request,
        data: &XdgShellSurfaceUserData,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        match request {
            xdg_popup::Request::Destroy => {
                if let Some(surface_data) = data.xdg_surface.data::<XdgSurfaceUserData>() {
                    surface_data.has_active_role.store(false, Ordering::Release);
                }
            }
            xdg_popup::Request::Grab { seat, serial } => {
                let handle = crate::wayland::shell::xdg::PopupSurface {
                    wl_surface: data.wl_surface.clone(),
                    shell_surface: popup.clone(),
                };

                let serial = Serial::from(serial);

                XdgShellHandler::grab(state, handle, seat, serial);
            }
            xdg_popup::Request::Reposition { positioner, token } => {
                let handle = crate::wayland::shell::xdg::PopupSurface {
                    wl_surface: data.wl_surface.clone(),
                    shell_surface: popup.clone(),
                };

                let positioner_data = *positioner
                    .data::<XdgPositionerUserData>()
                    .unwrap()
                    .inner
                    .lock()
                    .unwrap();

                XdgShellHandler::reposition_request(state, handle, positioner_data, token);
            }
            _ => unreachable!(),
        }
    }

    fn destroyed(state: &mut D, _client_id: ClientId, object_id: ObjectId, data: &XdgShellSurfaceUserData) {
        data.alive_tracker.destroy_notify();

        // remove this surface from the known ones (as well as any leftover dead surface)
        let mut shell_data = data.shell_data.lock().unwrap();
        if let Some(index) = shell_data
            .known_popups
            .iter()
            .position(|pop| pop.shell_surface.id() == object_id)
        {
            let popup = shell_data.known_popups.remove(index);
            drop(shell_data);
            XdgShellHandler::popup_destroyed(state, popup);
        }
    }
}

pub fn send_popup_configure(resource: &XdgPopup, configure: PopupConfigure) {
    let data = resource.data::<XdgShellSurfaceUserData>().unwrap();

    let serial = configure.serial;
    let geometry = configure.state.geometry;

    // Send repositioned if token is set
    if let Some(token) = configure.reposition_token {
        resource.repositioned(token);
    }

    // Send the popup configure
    resource.configure(geometry.loc.x, geometry.loc.y, geometry.size.w, geometry.size.h);

    // Send the base xdg_surface configure event to mark
    // the configure as finished
    data.xdg_surface.configure(serial.into());
}

pub fn make_popup_handle(resource: &XdgPopup) -> crate::wayland::shell::xdg::PopupSurface {
    let data = resource.data::<XdgShellSurfaceUserData>().unwrap();
    crate::wayland::shell::xdg::PopupSurface {
        wl_surface: data.wl_surface.clone(),
        shell_surface: resource.clone(),
    }
}
