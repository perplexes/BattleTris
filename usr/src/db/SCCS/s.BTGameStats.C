h47497
s 00000/00000/00000
d R 1.2 01/10/20 13:34:51 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTGameStats.C
c Name history : 1 0 src/db/BTGameStats.C
e
s 00095/00000/00000
d D 1.1 01/10/20 13:34:50 bmc 1 0
c date and time created 01/10/20 13:34:50 by bmc
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
/*    FILE: BTGameStats.C                                       */
/*    DATE: Thu May  5 03:45:56 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTGameStats.H"
#include "BTNetwork.H"

BTGameStats::BTGameStats()
: winnerScore_(0), winnerLines_(0), winnerFunds_(0),
  loserScore_(0), loserLines_(0), loserFunds_(0), duration_(0)
{
  bzero((char *) winnerName_, sizeof(winnerName_));
  bzero((char *) loserName_, sizeof(loserName_));
}

char *BTGameStats::writebuf(char *bufptr)
{
  unsigned long tl;

  bcopy((char *) winnerName_, (char *) bufptr, sizeof(winnerName_));
  bufptr += sizeof(winnerName_);

  BTNET_PUTLONG(bufptr, tl, winnerScore_);
  BTNET_PUTLONG(bufptr, tl, winnerLines_);
  BTNET_PUTLONG(bufptr, tl, winnerFunds_);

  bcopy((char *) loserName_, (char *) bufptr, sizeof(loserName_));
  bufptr += sizeof(loserName_);

  BTNET_PUTLONG(bufptr, tl, loserScore_);
  BTNET_PUTLONG(bufptr, tl, loserLines_);
  BTNET_PUTLONG(bufptr, tl, loserFunds_);

  BTNET_PUTLONG(bufptr, tl, duration_);

  return bufptr;
}

char *BTGameStats::readbuf(char *bufptr)
{
  unsigned long tl;

  bcopy((char *) bufptr, (char *) winnerName_, sizeof(winnerName_));
  bufptr += sizeof(winnerName_);

  BTNET_GETLONG(bufptr, tl, winnerScore_);
  BTNET_GETLONG(bufptr, tl, winnerLines_);
  BTNET_GETLONG(bufptr, tl, winnerFunds_);

  bcopy((char *) bufptr, (char *) loserName_, sizeof(loserName_));
  bufptr += sizeof(loserName_);

  BTNET_GETLONG(bufptr, tl, loserScore_);
  BTNET_GETLONG(bufptr, tl, loserLines_);
  BTNET_GETLONG(bufptr, tl, loserFunds_);

  BTNET_GETLONG(bufptr, tl, duration_);

  return bufptr;
}

BTGameStats::BTGameStats(const BTGameStats& other)
: winnerScore_(other.winnerScore_), winnerLines_(other.winnerLines_),
  winnerFunds_(other.winnerFunds_), loserScore_(other.loserScore_),
  loserLines_(other.loserLines_), loserFunds_(other.loserFunds_),
  duration_(other.duration_)
{
  strncpy(winnerName_, other.winnerName_, BTDBRECORD_KEYLEN);
  strncpy(loserName_, other.loserName_, BTDBRECORD_KEYLEN);
}

BTGameStats& BTGameStats::operator=(const BTGameStats& other)
{
  if(this == &other)
    return *this;

  winnerScore_ = other.winnerScore_;
  winnerLines_ = other.winnerLines_;
  winnerFunds_ = other.winnerFunds_;

  loserScore_ = other.loserScore_;
  loserLines_ = other.loserLines_;
  loserFunds_ = other.loserFunds_;

  strncpy(winnerName_, other.winnerName_, BTDBRECORD_KEYLEN);
  strncpy(loserName_, other.loserName_, BTDBRECORD_KEYLEN);

  duration_ = other.duration_;

  return *this;
}
E 1
