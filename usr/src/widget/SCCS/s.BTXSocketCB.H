h01436
s 00000/00000/00000
d R 1.2 01/10/20 13:35:22 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTXSocketCB.H
c Name history : 1 0 src/widget/BTXSocketCB.H
e
s 00027/00000/00000
d D 1.1 01/10/20 13:35:21 bmc 1 0
c date and time created 01/10/20 13:35:21 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: BTXSocketCB.H                                       */
/*    ASGN:                                                     */
/*    DATE: Fri Oct  6 15:48:58 1995                            */
/****************************************************************/


#ifndef BTXSOCKETCB_HEADER
#define BTXSOCKETCB_HEADER

#include "XtSocketCB.H"
#include "BTXDisplay.H"

class Socket;

class BTXSocketCB : public BTSocketCB {
public:
  BTXSocketCB( socketcb_t cb, void *data ) :
  cb_(((BTXDisplay *)DISPLAY)->app_, cb, data) {}

protected:
  XtSocketCB cb_;
};

#endif
E 1
