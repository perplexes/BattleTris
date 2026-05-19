h15730
s 00000/00000/00000
d R 1.2 01/10/20 13:35:01 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/sockets/XtSocketCB.C
c Name history : 1 0 src/sockets/XtSocketCB.C
e
s 00033/00000/00000
d D 1.1 01/10/20 13:35:00 bmc 1 0
c date and time created 01/10/20 13:35:00 by bmc
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
E 1
