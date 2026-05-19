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
