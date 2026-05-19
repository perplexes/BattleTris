h19670
s 00000/00000/00000
d R 1.2 01/10/20 13:34:52 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTDBErr.C
c Name history : 1 0 src/db/BTDBErr.C
e
s 00069/00000/00000
d D 1.1 01/10/20 13:34:51 bmc 1 0
c date and time created 01/10/20 13:34:51 by bmc
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
#include "BTDBErr.H"

static char errmsg[255];

char *BTDBErrMsg(short errcode)
{
  switch(errcode) {
  case ERRBTDBNOERR:
    strncpy(errmsg, "No error occurred", sizeof(errmsg));
    break;

  case ERRBTDBCREATE:
    strncpy(errmsg, "Failed to create file", sizeof(errmsg));
    break;

  case ERRBTDBOPEN:
    strncpy(errmsg, "Failed to open file", sizeof(errmsg));
    break;

  case ERRBTDBREMOVE:
    strncpy(errmsg, "Failed to unlink file", sizeof(errmsg));
    break;

  case ERRBTDBTIMEOUT:
    strncpy(errmsg, "Timed out waiting to lock file", sizeof(errmsg));
    break;

  case ERRBTDBCLOSE:
    strncpy(errmsg, "Failed to close file", sizeof(errmsg));
    break;

  case ERRBTDBREAD:
    strncpy(errmsg, "Failed to read from file", sizeof(errmsg));
    break;

  case ERRBTDBWRITE:
    strncpy(errmsg, "Failed to write to file", sizeof(errmsg));
    break;

  case ERRBTDBSTAT:
    strncpy(errmsg, "Failed to stat file", sizeof(errmsg));	
    break;

  case ERRBTDBLOCK:
    strncpy(errmsg, "Failed to lock byte range", sizeof(errmsg));
    break;

  case ERRBTDBSEEK:
    strncpy(errmsg, "Failed to seek file", sizeof(errmsg));
    break;

  case ERRBTDBCORRUPT:
    strncpy(errmsg, "Database file appears to be corrupt", sizeof(errmsg));
    break;

  default:
    strncpy(errmsg, "Unknown database error", sizeof(errmsg));
  }

  return errmsg;
}
E 1
