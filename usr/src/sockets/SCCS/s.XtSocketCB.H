h48943
s 00000/00000/00000
d R 1.2 01/10/20 13:35:00 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/sockets/XtSocketCB.H
c Name history : 1 0 src/sockets/XtSocketCB.H
e
s 00051/00000/00000
d D 1.1 01/10/20 13:34:59 bmc 1 0
c date and time created 01/10/20 13:34:59 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: XtSocketCB.H                                        */
/*    DATE: Sat Dec  3 12:20:27 1994                            */
/****************************************************************/

#ifndef XTSOCKETCB_H
#define XTSOCKETCB_H

#include "BTConfig.H"

#include <X11/Intrinsic.h>

#include "SocketCB.H"

class XtSocketCB : public SocketCB {

private:
  static void callback(XtPointer data, int *fd, XtInputId *id) {
    SocketCBData *cbdata = (SocketCBData *) data;
    cbdata->cb_->docall(cbdata);
  }

protected:
  XtAppContext ctx_;

protected:
  void install(SocketCBData *cbdata);
  void remove(SocketCBData *cbdata) {
    XtRemoveInput((XtInputId) cbdata->data_);
  }

public:
  XtSocketCB(XtAppContext ctx, socketcb_t callback, void *data)
  : SocketCB(callback, data), ctx_(ctx) {}

  XtSocketCB(const XtSocketCB& other)
  : SocketCB(*((SocketCB *) &other)), ctx_(other.ctx_) {}

  ~XtSocketCB() {}

  XtSocketCB& operator=(const XtSocketCB& other) {
    if(this == &other)
      return *this;
    SocketCB::operator=(*((SocketCB *) &other));
    return *this;
  }
};

#endif
E 1
