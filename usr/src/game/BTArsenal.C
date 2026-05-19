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

char *
BTArsenal::getName(int index)
{
	if (quantity_[index]) {
		return (rep_[index]->name_);
	} else {
		return ((char *)"< Empty >");
	}
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

