fn window() {

struct wl_display *display = wl_display_connect(NULL);
struct wl_surface *surface = wl_compositor_create_surface(compositor);


struct wl_shell_surface *shell_surface = wl_shell_get_shell_surface(shell, surface);
wl_shell_surface_set_toplevel(shell_surface);

struct wl_buffer *buffer = /* create buffer from shm pool */;
wl_surface_attach(surface, buffer, 0, 0);
wl_surface_commit(surface);



}
