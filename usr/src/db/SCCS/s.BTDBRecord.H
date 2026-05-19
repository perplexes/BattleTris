h31126
s 00000/00000/00000
d R 1.2 01/10/20 13:34:48 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTDBRecord.H
c Name history : 1 0 src/db/BTDBRecord.H
e
s 00043/00000/00000
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
/*    FILE: BTDBRecord.H                                        */
/*    DATE: Sat Sep  3 18:05:47 1994                            */
/****************************************************************/

#ifndef BTDBRECORD_H
#define BTDBRECORD_H

#include "BTConfig.H"

enum {
  BTDBRECORD_KEYLEN = 280
};

class BTDBRecord {

protected:
  char key_[BTDBRECORD_KEYLEN + 1];
  off_t size_;
  int valid_;

public:
  BTDBRecord(const char *key);
  virtual ~BTDBRecord() {}

  operator int() const { return valid_; }
  operator !() const { return !valid_; }

  virtual short read(int fd, off_t offset, off_t nbytes) = 0;
  virtual short write(int fd, off_t offset) = 0;

  const char *key() const { return (const char *) key_; }
  char *key() { return key_; }
  off_t size() const { return size_; }

  BTDBRecord(const BTDBRecord& other);
  BTDBRecord& operator=(const BTDBRecord& other);

};

#endif
E 1
