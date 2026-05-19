h31192
s 00000/00000/00000
d R 1.2 01/10/20 13:34:49 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTDBReadLock.H
c Name history : 1 0 src/db/BTDBReadLock.H
e
s 00044/00000/00000
d D 1.1 01/10/20 13:34:48 bmc 1 0
c CodeManager Uniquification: src/db/BTDBReadLock.H
c date and time created 01/10/20 13:34:48 by bmc
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
/*    FILE: BTDBReadLock.H                                      */
/*    DATE: Sat Sep  3 21:45:24 1994                            */
/****************************************************************/

#ifndef BTDBREADLOCK_H
#define BTDBREADLOCK_H

#include "BTConfig.H"
#include "BTDBLock.H"

class BTDBReadLock : public BTDBLock {

public:
  BTDBReadLock(int filedes, off_t offset = 0, off_t length = 0)
  : BTDBLock(filedes, offset, length) {}

  short lock() {
    short err = lockRegion(filedes_, F_SETLK, F_RDLCK, offset_, length_);

    if(err == ERRBTDBNOERR)
      locked_ = 1;

    return err;
  }

  short lockw() {
    short err = lockRegion(filedes_, F_SETLKW, F_RDLCK, offset_, length_);

    if(err == ERRBTDBNOERR)
      locked_ = 1;

    return err;
  }

  pid_t testLock() {
    return testRegion(filedes_, F_RDLCK, offset_, length_);
  }

};

#endif
E 1
