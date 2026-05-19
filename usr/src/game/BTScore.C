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

