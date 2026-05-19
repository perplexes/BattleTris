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
