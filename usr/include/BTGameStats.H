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
