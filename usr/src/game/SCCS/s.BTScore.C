h32742
s 00000/00000/00000
d R 1.2 01/10/20 13:35:35 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTScore.C
c Name history : 1 0 src/game/BTScore.C
e
s 00041/00000/00000
d D 1.1 01/10/20 13:35:34 bmc 1 0
c date and time created 01/10/20 13:35:34 by bmc
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
/*    FILE: BTScore.C                                           */
/*    DATE: Thu Dec 29 00:32:06 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTNetwork.H"
#include "BTScore.H"

char *BTScore::writebuf(char *bufptr)
{
  unsigned long tl;
  long tmp;

  BTNET_PUTLONG(bufptr, tl, score_);
  BTNET_PUTLONG(bufptr, tl, op_score_);
  BTNET_PUTLONG(bufptr, tl, lines_);
  BTNET_PUTLONG(bufptr, tl, op_lines_);
  BTNET_PUTLONG(bufptr, tmp, funds_);
  BTNET_PUTLONG(bufptr, tmp, op_funds_);

  return bufptr;
}

char *BTScore::readbuf(char *bufptr)
{
  unsigned long tl;
  long tmp;

  BTNET_GETLONG(bufptr, tl, score_);
  BTNET_GETLONG(bufptr, tl, op_score_);
  BTNET_GETLONG(bufptr, tl, lines_);
  BTNET_GETLONG(bufptr, tl, op_lines_);
  BTNET_GETLONG(bufptr, tmp, funds_);
  BTNET_GETLONG(bufptr, tmp, op_funds_);

  return bufptr;
}

E 1
