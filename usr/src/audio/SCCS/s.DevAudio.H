h59890
s 00012/00014/00022
d D 1.2 01/10/21 19:25:02 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:07 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/audio/DevAudio.H
c Name history : 1 0 src/audio/DevAudio.H
e
s 00036/00000/00000
d D 1.1 01/10/20 13:35:06 bmc 1 0
c date and time created 01/10/20 13:35:06 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: DevAudio.H                                          */
/*    ASSN:                                                     */
/*    DATE: Thu Feb 10 14:48:18 1994                            */
/****************************************************************/

#ifndef DEVAUDIO_H
#define DEVAUDIO_H

#include "BTConfig.H"

class DevAudio {
D 3

E 3
private:
D 3
  int fd_, slave_fd_, master_fd_;
  pid_t slave_pid_;
  char root_[1024];
  int valid_;
E 3
I 3
	int fd_, slave_fd_, master_fd_;
	pid_t slave_pid_;
	char root_[1024];
	int valid_;
E 3

public:
D 3
  DevAudio(int headphones = 0, char *root = 0);
  ~DevAudio();
E 3
I 3
	DevAudio(int headphones = 0, const char *root = 0);
	~DevAudio();
E 3

D 3
  void QueueFile(char *filename);
  void DumpFile(char *filename);
  void SlaveLoop();
E 3
I 3
	void QueueFile(const char *filename);
	void DumpFile(const char *filename);
	void SlaveLoop();
	pid_t slavePid() { return slave_pid_; }
E 3

D 3
  pid_t slavePid() { return slave_pid_; }

  void operator<<(char *f) { QueueFile (f); }
  operator int() { return valid_; }
E 3
I 3
	void operator<< (const char *f) { QueueFile (f); }
	operator int() { return valid_; }
E 3
};

#endif
E 1
