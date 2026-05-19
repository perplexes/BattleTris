h46150
s 00000/00000/00000
d R 1.2 01/10/20 13:35:02 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/sockets/StreamSocketErr.C
c Name history : 1 0 src/sockets/StreamSocketErr.C
e
s 00063/00000/00000
d D 1.1 01/10/20 13:35:01 bmc 1 0
c date and time created 01/10/20 13:35:01 by bmc
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
/*    FILE: StreamSocketErr.C                                   */
/*    DATE: Wed Jan 26 23:50:16 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "StreamSocketErr.H"

static char errmsg[255];

char *StreamSocketErrMsg(short errcode)
{
  switch(errcode) {
  case ERRSTREAMNOERR:
    strncpy(errmsg, "No error occurred", sizeof(errmsg));
    break;
  case ERRSTREAMBIND:
    strncpy(errmsg, "Failed to bind name to socket", sizeof(errmsg));
    break;
  case ERRSTREAMNAME:
    strncpy(errmsg, "Failed to obtain name for socket", sizeof(errmsg));
    break;
  case ERRSTREAMCLOSE:
    strncpy(errmsg, "Failed to close socket", sizeof(errmsg));
    break;
  case ERRSTREAMCONNECT:
    strncpy(errmsg, "Failed to connect socket", sizeof(errmsg));
    break;
  case ERRSTREAMLISTEN:
    strncpy(errmsg, "Failed to enable listening", sizeof(errmsg));
    break;
  case ERRSTREAMSELECT:
    strncpy(errmsg, "Failed to select on socket", sizeof(errmsg));
    break;
  case ERRSTREAMACCEPT:
    strncpy(errmsg, "Failed to accept connection", sizeof(errmsg));
    break;
  case ERRSTREAMSEND:
    strncpy(errmsg, "Failed to send data", sizeof(errmsg));
    break;
  case ERRSTREAMRECV:
    strncpy(errmsg, "Failed to receive data", sizeof(errmsg));
    break;
  case ERRSTREAMFLAG:
    strncpy(errmsg, "Failed to set socket flag", sizeof(errmsg));
    break;
  case ERRSTREAMOPT:
    strncpy(errmsg, "Failed to set socket option", sizeof(errmsg));
    break;
  case ERRSTREAMBROKEN:
    strncpy(errmsg, "Connection unexpectedly broke", sizeof(errmsg));
    break;
  case ERRSTREAMTIMEOUT:
    strncpy(errmsg, "Timed out waiting for data", sizeof(errmsg));
    break;
  default:
    strncpy(errmsg, "Unknown StreamSocket error", sizeof(errmsg));
  }

  return errmsg;
}
E 1
