h12146
s 00000/00000/00000
d R 1.2 01/10/20 13:34:50 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTPlayer.H
c Name history : 1 0 src/db/BTPlayer.H
e
s 00091/00000/00000
d D 1.1 01/10/20 13:34:49 bmc 1 0
c date and time created 01/10/20 13:34:49 by bmc
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
/*    FILE: BTPlayer.H                                          */
/*    DATE: Thu Apr 21 16:32:33 1994                            */
/****************************************************************/

#ifndef BTPLAYER_H
#define BTPLAYER_H

#include "BTConfig.H"

#ifndef NDEBUG
# include <iostream.h>
#endif

#include "BTConstants.H"
#include "BTDBRecord.H"

enum BTStreakType {
  BTSTREAK_WINS,
  BTSTREAK_LOSSES,
  BTSTREAK_EMPTY
};

class BTPlayerRecord;
class BTGameStats;

class BTPlayer : public BTDBRecord {

#ifndef NDEBUG
friend ostream& operator<<(ostream& os, BTPlayer& player);
#endif

public:
  char gecos_[BT_GECOSNAMELEN + 1];
  unsigned long rank_;
  unsigned long wins_;
  unsigned long losses_;
  unsigned long highScore_;
  unsigned long highLines_;
  unsigned long highFunds_;
  unsigned long streak_;
  unsigned short streakType_;
  unsigned long fastestKill_;
  unsigned long quickestDeath_;
  unsigned long longestGame_;

public:
  BTPlayerRecord **records_;
  unsigned short rsize_;

private:
  void validateBuffer(char *buf, int buflen);
  void concatTime(char *buf, long secs);

public:
  BTPlayer(const char *name = "");
  ~BTPlayer();

  short read(int fd, off_t offset, off_t nbytes);
  short write(int fd, off_t offset);

  char *writebuf(char *bufptr);
  char *readbuf(char *bufptr);

  size_t datalen() {
    return sizeof(gecos_) + sizeof(rank_) + sizeof(wins_) + sizeof(losses_) +
      sizeof(highScore_) + sizeof(highLines_) + sizeof(highFunds_) +
      sizeof(streak_) + sizeof(streakType_) + sizeof(fastestKill_) +
      sizeof(quickestDeath_) + sizeof(longestGame_) + sizeof(rsize_);
  }

  void recordWin(BTGameStats *stats, BTPlayer *opponent);
  void recordLoss(BTGameStats *stats, BTPlayer *opponent);

  char *formatInfo();
  BTPlayerRecord *recordAgainst(BTPlayer *opponent);
  int plan(char *nickname, int namelen, char *plan, int planlen);

  int operator==(const BTPlayer& other);
  int operator!=(const BTPlayer& other);

  static int compareName(const void *left, const void *right);
  static int compareRank(const void *left, const void *right);

  BTPlayer(const BTPlayer& other);
  BTPlayer& operator=(const BTPlayer& other);
};

#endif
E 1
