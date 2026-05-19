h41440
s 00000/00000/00000
d R 1.2 01/10/20 13:34:51 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTGameStats.H
c Name history : 1 0 src/db/BTGameStats.H
e
s 00044/00000/00000
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
/*    FILE: BTGameStats.H                                       */
/*    DATE: Thu May  5 03:41:22 1994                            */
/****************************************************************/

#ifndef BTGAMESTATS_H
#define BTGAMESTATS_H

#include "BTConfig.H"
#include "BTDBRecord.H"

class BTGameStats {

public:
  char winnerName_[BTDBRECORD_KEYLEN + 1];
  unsigned long winnerScore_;
  unsigned long winnerLines_;
  unsigned long winnerFunds_;
  char loserName_[BTDBRECORD_KEYLEN + 1];
  unsigned long loserScore_;
  unsigned long loserLines_;
  unsigned long loserFunds_;
  unsigned long duration_;

public:
  BTGameStats();
  ~BTGameStats() {}

  char *writebuf(char *bufptr);
  char *readbuf(char *bufptr);
  
  size_t datalen() {
    return ((sizeof(winnerName_) + sizeof(winnerScore_) +
	     sizeof(winnerLines_) + sizeof(winnerFunds_)) * 2) +
	     sizeof(duration_);
  }

  BTGameStats(const BTGameStats& other);
  BTGameStats& operator=(const BTGameStats& other);
};

#endif
E 1
