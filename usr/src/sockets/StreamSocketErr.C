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
