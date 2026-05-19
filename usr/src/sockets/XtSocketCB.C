/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: XtSocketCB.C                                        */
/*    DATE: Sat Dec  3 13:05:13 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "XtSocketCB.H"
#include "Socket.H"

void XtSocketCB::install(SocketCBData *cbdata)
{
  XtInputMask mask;

  switch(cbdata->reason_) {

  SOCKET_CB_WRITE:
    mask = XtInputWriteMask;
    break;

  SOCKET_CB_EXCEPT:
    mask = XtInputExceptMask;
    break;

  default:
    mask = XtInputReadMask;
    break;
  }

  cbdata->data_ = XtAppAddInput(ctx_, cbdata->sock_->sock(), (XtPointer) mask,
	(XtInputCallbackProc) XtSocketCB::callback, (XtPointer) cbdata);
}
