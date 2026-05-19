h19488
s 00000/00000/00000
d R 1.2 01/10/20 13:34:50 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTPlayerRecord.C
c Name history : 1 0 src/db/BTPlayerRecord.C
e
s 00064/00000/00000
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
/*    FILE: BTPlayerRecord.C                                    */
/*    DATE: Thu May  5 01:46:53 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <assert.h>

#include "BTPlayerRecord.H"

BTPlayerRecord::BTPlayerRecord(const BTPlayerRecord& other)
{
  bzero(opponent_, sizeof(opponent_));
  strncpy(opponent_, other.opponent_, BTDBRECORD_KEYLEN);
  wins_ = other.wins_;
  losses_ = other.losses_;
}

BTPlayerRecord& BTPlayerRecord::operator=(const BTPlayerRecord& other)
{
  if(this == &other)
    return *this;

  strncpy(opponent_, other.opponent_, BTDBRECORD_KEYLEN);
  wins_ = other.wins_;
  losses_ = other.losses_;

  return *this;
}

int BTPlayerRecord::operator==(const BTPlayerRecord& other)
{
  return strcmp(opponent_, other.opponent_) == 0;
}

int BTPlayerRecord::operator!=(const BTPlayerRecord& other)
{
  return strcmp(opponent_, other.opponent_) != 0;
}

int BTPlayerRecord::compare(const void *left, const void *right)
{
  assert(left != 0);
  assert(right != 0);

  BTPlayerRecord *lval = *((BTPlayerRecord **) left);
  BTPlayerRecord *rval = *((BTPlayerRecord **) right);

  assert(lval != 0);
  assert(rval != 0);

  return strcmp(lval->opponent_, rval->opponent_);
}

#ifndef NDEBUG
ostream& operator<<(ostream& os, BTPlayerRecord& record)
{
  os << "Record against " << record.opponent_ << ":\n";
  return os << record.wins_ << " wins, " << record.losses_ << " losses\n";
}
#endif
E 1
