h39438
s 00000/00000/00000
d R 1.2 01/10/20 13:34:48 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTNetworkEntry.H
c Name history : 1 0 src/db/BTNetworkEntry.H
e
s 00072/00000/00000
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
/*    FILE: BTNetworkEntry.H                                    */
/*    DATE: Thu Apr 21 16:16:28 1994                            */
/****************************************************************/

#ifndef BTNETWORKENTRY_H
#define BTNETWORKENTRY_H

#include "BTConfig.H"

#include <iostream.h>

#include "Address.H"
#include "BTDBRecord.H"
#include "BTConstants.H"

enum BTNetworkStatus
{
  BTSTATUS_UNKNOWN,
  BTSTATUS_WAITING,
  BTSTATUS_PLAYING
};

class BTNetworkEntry : public BTDBRecord {

friend ostream& operator<<(ostream& os, BTNetworkEntry& entry);

public:
  char userName_[BT_USERNAMELEN + 1];
  char hostName_[BT_HOSTNAMELEN + 1];
  unsigned long timestamp_;
  unsigned long pid_;
  unsigned long addrnet_;
  unsigned long addrlna_;
  unsigned short port_;
  unsigned short maxwpn_;
  unsigned short majver_;
  unsigned short minver_;
  unsigned short status_;

public:
  BTNetworkEntry();
  BTNetworkEntry(char *userName, char *hostName, InetAddress& addr);
  ~BTNetworkEntry() {}

  short read(int fd, off_t offset, off_t nbytes);
  short write(int fd, off_t offset);

  char *writebuf(char *bufptr);
  char *readbuf(char *bufptr);

  size_t datalen() {
    return sizeof(userName_) + sizeof(hostName_) +
      sizeof(timestamp_) + sizeof(pid_) +
      sizeof(addrnet_) + sizeof(addrlna_) + sizeof(port_) +
      sizeof(maxwpn_) + sizeof(majver_) + sizeof(minver_) + sizeof(status_);
  }

  void touch() { timestamp_ = time(0); }

  int operator==(const BTNetworkEntry& other);
  int operator!=(const BTNetworkEntry& other);

  static int compare(const void *left, const void *right);

  BTNetworkEntry(const BTNetworkEntry& other);
  BTNetworkEntry& operator=(const BTNetworkEntry& other);
};

#endif
E 1
