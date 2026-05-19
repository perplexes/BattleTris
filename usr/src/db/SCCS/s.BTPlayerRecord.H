h53038
s 00000/00000/00000
d R 1.2 01/10/20 13:34:50 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTPlayerRecord.H
c Name history : 1 0 src/db/BTPlayerRecord.H
e
s 00052/00000/00000
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
/*    FILE: BTPlayerRecord.H                                    */
/*    DATE: Thu Apr 21 16:37:52 1994                            */
/****************************************************************/

#ifndef BTPLAYERRECORD_H
#define BTPLAYERRECORD_H

#include "BTConfig.H"

#ifndef NDEBUG
# include <iostream.h>
#endif

#include "BTDBRecord.H"

class BTPlayerRecord {

#ifndef NDEBUG
friend ostream& operator<<(ostream& os, BTPlayerRecord& record);
#endif

public:
  char opponent_[BTDBRECORD_KEYLEN + 1];
  unsigned long wins_;
  unsigned long losses_;

public:
  BTPlayerRecord(const char *opponent = "")
  : wins_(0), losses_(0) {
    bzero(opponent_, sizeof(opponent_));
    strncpy(opponent_, opponent, BTDBRECORD_KEYLEN);
  }

  ~BTPlayerRecord() {}

  size_t datalen() {
    return sizeof(opponent_) + sizeof(wins_) + sizeof(losses_);
  }

  int operator==(const BTPlayerRecord& other);
  int operator!=(const BTPlayerRecord& other);

  static int compare(const void *left, const void *right);

  BTPlayerRecord(const BTPlayerRecord& other);
  BTPlayerRecord& operator=(const BTPlayerRecord& other);
};

#endif
E 1
