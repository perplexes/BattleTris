h27192
s 00000/00000/00000
d R 1.2 01/10/20 13:34:48 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTDBRecord.C
c Name history : 1 0 src/db/BTDBRecord.C
e
s 00041/00000/00000
d D 1.1 01/10/20 13:34:47 bmc 1 0
c date and time created 01/10/20 13:34:47 by bmc
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
/*    FILE: BTDBRecord.C                                        */
/*    DATE: Sat Sep  3 22:30:46 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <assert.h>

#include "BTDBRecord.H"

BTDBRecord::BTDBRecord(const char *key)
: valid_(0), size_(sizeof(key_))
{
  assert(key != 0);

  bzero((char *) key_, sizeof(key_));
  strncpy(key_, key, BTDBRECORD_KEYLEN);
}

BTDBRecord::BTDBRecord(const BTDBRecord& other)
{
  bzero((char *) key_, sizeof(key_));
  strncpy(key_, other.key_, BTDBRECORD_KEYLEN);
  size_ = other.size_;
  valid_ = other.valid_;
}

BTDBRecord& BTDBRecord::operator=(const BTDBRecord& other)
{
  if(&other == this)
    return *this;

  strncpy(key_, other.key_, BTDBRECORD_KEYLEN);
  size_ = other.size_;
  valid_ = other.valid_;

  return *this;
}
E 1
