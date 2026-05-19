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
