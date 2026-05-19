h19519
s 00000/00000/00000
d R 1.2 01/10/20 13:35:01 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/sockets/Address.C
c Name history : 1 0 src/sockets/Address.C
e
s 00175/00000/00000
d D 1.1 01/10/20 13:35:00 bmc 1 0
c date and time created 01/10/20 13:35:00 by bmc
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
/*    FILE: Address.C                                           */
/*    DATE: Fri Apr 15 20:00:18 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <assert.h>
#include <netdb.h>

#include "Address.H"

void Address::addr(sockaddr *addr, int size)
{
  assert(addr != 0);
  assert(size > 0);
  assert(size <= sizeof(sockaddr));

  bzero((char *) addr_, sizeof(sockaddr));
  bcopy((char *) addr, (char *) addr_, size);
  valid_ = 1;
}

InetAddress::InetAddress()
: Address((sockaddr *) new sockaddr_in, sizeof(sockaddr_in))
{
  sockaddr_in *address = (sockaddr_in *) Address::addr();
  bzero((char *) address, sizeof(sockaddr_in));
 
  address->sin_family = AF_INET;
  address->sin_addr.s_addr = htonl(INADDR_ANY);
  address->sin_port = htons(0);
}

InetAddress::InetAddress(unsigned short port, const char *host)
: Address((sockaddr *) new sockaddr_in, sizeof(sockaddr_in))
{
  sockaddr_in *address = (sockaddr_in *) Address::addr();
  bzero((char *) address, sizeof(sockaddr_in));

  if(host) {
    if(!validateHostAddr(host))
      return;

    address->sin_family = AF_INET;
    address->sin_port = htons(port);
  } else {
    address->sin_family = AF_INET;
    address->sin_addr.s_addr = htonl(INADDR_ANY);
    address->sin_port = htons(port);
  }
}

InetAddress::InetAddress(unsigned short port, unsigned long net,
			 unsigned long lna)
: Address((sockaddr *) new sockaddr_in, sizeof(sockaddr_in))
{
  sockaddr_in *address = (sockaddr_in *) Address::addr();
  bzero((char *) address, sizeof(sockaddr_in));

  address->sin_family = AF_INET;
  address->sin_addr = inet_makeaddr(net, lna);
  address->sin_port = htons(port);
}

InetAddress::InetAddress(const sockaddr_in& addr)
: Address((sockaddr *) new sockaddr_in, sizeof(sockaddr_in))
{
  sockaddr_in *address = (sockaddr_in *) Address::addr();
  bcopy((char *) &addr, (char *) address, sizeof(sockaddr_in));
}

InetAddress::InetAddress(const InetAddress& addr)
: Address((sockaddr *) new sockaddr_in, sizeof(sockaddr_in))
{
  sockaddr_in *address = (sockaddr_in *) Address::addr();
  bcopy((char *) addr.addr(), (char *) address, sizeof(sockaddr_in));
  Address::valid(addr.valid());
}

char *InetAddress::hostName()
{
  sockaddr_in *address = (sockaddr_in *) Address::addr();

  hostent *hostinfo = gethostbyaddr((char *) &(address->sin_addr),
				    sizeof(in_addr), address->sin_family);

  if(hostinfo)
    return hostinfo->h_name;

  return (char *) 0;
}

void InetAddress::hostName(const char *host)
{
  assert(host != 0);
  validateHostAddr(host);
}

InetAddress& InetAddress::operator=(const InetAddress& other)
{
  if(this == &other)
    return *this;

  sockaddr_in *address = (sockaddr_in *) Address::addr();
  bcopy((char *) other.addr(), (char *) address, sizeof(sockaddr_in));
  Address::valid(other.valid());

  return *this;
}

int InetAddress::validateHostAddr(const char *host)
{
  assert(host != 0);

  hostent *hostinfo = gethostbyname(host);
  if(hostinfo == 0) {
    Address::valid(0);
    return 0;
  }

  sockaddr_in *address = (sockaddr_in *) addr();
  bcopy((char *) hostinfo->h_addr, (char *) &address->sin_addr,
        hostinfo->h_length);

  return 1;
}

UnixAddress::UnixAddress()
: Address((sockaddr *) new sockaddr_un, sizeof(sockaddr_un))
{
  sockaddr_un *address = (sockaddr_un *) Address::addr();
  bzero((char *) address, sizeof(sockaddr_un));
  address->sun_family = AF_UNIX;
  Address::valid(0);
}

UnixAddress::UnixAddress(const char *pathname)
: Address((sockaddr *) new sockaddr_un, sizeof(sockaddr_un))
{
  assert(pathname != 0);
  sockaddr_un *address = (sockaddr_un *) Address::addr();
  bzero((char *) address, sizeof(sockaddr_un));
  address->sun_family = AF_UNIX;
  strncpy(address->sun_path, pathname, sizeof(address->sun_path) - 1);
}

UnixAddress::UnixAddress(const sockaddr_un& addr)
: Address((sockaddr *) new sockaddr_un, sizeof(sockaddr_un))
{
  sockaddr_un *address = (sockaddr_un *) Address::addr();
  bcopy((char *) &addr, (char *) address, sizeof(sockaddr_un));
}

UnixAddress::UnixAddress(const UnixAddress& addr)
: Address((sockaddr *) new sockaddr_un, sizeof(sockaddr_un))
{
  sockaddr_un *address = (sockaddr_un *) Address::addr();
  bcopy((char *) addr.addr(), (char *) address, sizeof(sockaddr_un));
  Address::valid(addr.valid());
}

UnixAddress& UnixAddress::operator=(const UnixAddress& other)
{
  if(this == &other)
    return *this;

  sockaddr_un *address = (sockaddr_un *) Address::addr();
  bcopy((char *) other.addr(), (char *) address, sizeof(sockaddr_un));
  Address::valid(other.valid());

  return *this;
}
E 1
