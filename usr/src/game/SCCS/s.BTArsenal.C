h55396
s 00008/00005/00049
d D 1.2 01/10/21 19:25:03 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:22 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTArsenal.C
c Name history : 1 0 src/game/BTArsenal.C
e
s 00054/00000/00000
d D 1.1 01/10/20 13:35:21 bmc 1 0
c date and time created 01/10/20 13:35:21 by bmc
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
/*    FILE: BTArsenal.C                                         */
/*    ASSN:                                                     */
/*    DATE: Fri Apr 29 22:13:01 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTArsenal.H"

BTArsenal::BTArsenal() {
  clear();
}

D 3
char *BTArsenal::getName(int index) {
  if (quantity_[index]) 
    return rep_[index]->name_;
  else
    return "< Empty >";
E 3
I 3
char *
BTArsenal::getName(int index)
{
	if (quantity_[index]) {
		return (rep_[index]->name_);
	} else {
		return ((char *)"< Empty >");
	}
E 3
}

int BTArsenal::buyWeapon (BTWeapon *data) {
  register int i;

  for (i = 0; i < BT_ARSENAL_SIZE; i++) {
    if (rep_[i] == data) {
      quantity_[i]++;
      break;
    }
    if (!rep_[i]) {
      rep_[i] = data;
      quantity_[i]++;
      break;
    }
  }
  if (i == BT_ARSENAL_SIZE)
    return 0;
  return 1;
}

void BTArsenal::useWeapon (int index) {
  quantity_[index]--;
  if (!quantity_[index])
    rep_[index] = 0;
}

void BTArsenal::clear() {
  for (int i = 0; i < BT_ARSENAL_SIZE; i++) {
    rep_[i] = 0;
    quantity_[i] = 0;
  }
}

E 1
