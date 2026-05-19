/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: Address.H                                           */
/*    DATE: Mon Feb  7 15:39:58 1994                            */
/****************************************************************/

#ifndef ADDRESS_H
#define ADDRESS_H

#include "BTConfig.H"

#include <sys/socket.h>
#include <sys/un.h>
#include <netinet/in.h>
#include <arpa/inet.h>

class Address {
private:
  sockaddr *addr_;
  int size_;
  int valid_;

protected:
  Address(sockaddr *addr, int size) : addr_(addr), size_(size), valid_(1) {}
  int valid() const { return valid_; }
  void valid(int isValid) { valid_ = isValid; }

public:
  virtual ~Address() { delete addr_; }

  operator int() const { return valid_; }
  // operator !() const { return !valid_; }

  int& size() { return size_; }
  int size() const { return size_; }

  const sockaddr *addr() const { return addr_; }
  sockaddr *addr() { return addr_; }

  void addr(sockaddr *addr, int size);
};

class InetAddress : public Address {
private:
  int validateHostAddr(const char *host);

public:
  InetAddress();
  InetAddress(unsigned short port, const char* host = 0);
  InetAddress(unsigned short port, unsigned long net, unsigned long lna);
  InetAddress(const sockaddr_in& addr);
  InetAddress(const InetAddress& addr);

  virtual ~InetAddress() {} 

  unsigned short port() {
    return ntohs(((sockaddr_in*) addr())->sin_port);
  }

  unsigned long net() {
    return (unsigned long) inet_netof(((sockaddr_in*) addr())->sin_addr);
  }

  unsigned long lna() {
    return (unsigned long) inet_lnaof(((sockaddr_in*) addr())->sin_addr);
  }

  void port(unsigned short port) {
    ((sockaddr_in*) addr())->sin_port = htons(port);
  }

  unsigned long hostAddr() {
    return ntohl(((sockaddr_in*) addr())->sin_addr.s_addr);
  }

  void hostAddr(unsigned long address) {
    ((sockaddr_in*) addr())->sin_addr.s_addr = htonl(address);
  }

  void hostName(const char *host);
  char *hostName();

  char *addressName() {
    return inet_ntoa(((sockaddr_in*) addr())->sin_addr);
  }

  InetAddress& operator=(const InetAddress& addr);
};

class UnixAddress : public Address {
public:
  UnixAddress(void);
  UnixAddress(const char *pathname);
  UnixAddress(const sockaddr_un& addr);
  UnixAddress(const UnixAddress& addr);

  virtual ~UnixAddress() {}

  char *path() {
    return ((sockaddr_un *) addr())->sun_path;
  }

  void path(const char *pathname) {
    strncpy(((sockaddr_un *) addr())->sun_path, pathname,
      sizeof(((sockaddr_un *) addr())->sun_path) - 1);
    Address::valid(1);
  }

  UnixAddress& operator=(const UnixAddress& addr);
};

#endif
