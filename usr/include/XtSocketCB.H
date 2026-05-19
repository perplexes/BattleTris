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
