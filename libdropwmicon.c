#define _GNU_SOURCE
#include <dlfcn.h>
#include <X11/Xlib.h>
#include <X11/Xatom.h>

static int (*real_XChangeProperty)(Display*,Window,Atom,Atom,int,int,const unsigned char*,int);

int XChangeProperty(Display* dpy, Window w, Atom property, Atom type,
                    int format, int mode, const unsigned char* data, int nelements) {
    if (!real_XChangeProperty)
        real_XChangeProperty = dlsym(RTLD_NEXT, "XChangeProperty");

    if (format == 32) {
        static Atom net_wm_icon = None;
        if (net_wm_icon == None)
            net_wm_icon = XInternAtom(dpy, "_NET_WM_ICON", False);
        /* Drop absurdly large icon payloads that trip Xwayland */
        if (property == net_wm_icon && nelements > 60000)
            return 1; /* pretend success */
    }
    return real_XChangeProperty(dpy, w, property, type, format, mode, data, nelements);
}
