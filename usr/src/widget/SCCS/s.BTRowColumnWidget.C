h55709
s 00028/00019/00022
d D 1.2 01/10/21 19:25:17 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:15 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTRowColumnWidget.C
c Name history : 1 0 src/widget/BTRowColumnWidget.C
e
s 00041/00000/00000
d D 1.1 01/10/20 13:35:14 bmc 1 0
c date and time created 01/10/20 13:35:14 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTRowColumnWidget.C                                 */
/*    ASGN: Final                                               */
/*    DATE: Thu Apr 21 14:18:44 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTRowColumnWidget.H"

D 3
BTRowColumnWidget::BTRowColumnWidget(BTWidget *parent, char *const name,
                                     Position x1, Position y1,
                                     Position x2, Position y2)
E 3
I 3
BTRowColumnWidget::BTRowColumnWidget(BTWidget *parent, const char *name,
    Position x1, Position y1, Position x2, Position y2)
E 3
: BTWidget(parent)
{
D 3
  me_ =
    XtVaCreateWidget(name, xmRowColumnWidgetClass, parent ? parent->getWidget() : 0,
                     XmNtopAttachment, XmATTACH_POSITION, XmNtopPosition, y1,
                     XmNleftAttachment, XmATTACH_POSITION, XmNleftPosition, x1,
                     XmNrightAttachment, XmATTACH_POSITION, XmNrightPosition,x2,
                     XmNbottomAttachment, XmATTACH_POSITION,
                     XmNbottomPosition, y2, NULL);
E 3
I 3
	me_ = XtVaCreateWidget(name, xmRowColumnWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNtopAttachment, XmATTACH_POSITION,
	    XmNtopPosition, y1,
	    XmNleftAttachment, XmATTACH_POSITION,
	    XmNleftPosition, x1,
	    XmNrightAttachment, XmATTACH_POSITION,
	    XmNrightPosition,x2,
	    XmNbottomAttachment, XmATTACH_POSITION,
	    XmNbottomPosition, y2,
	    NULL);
E 3
}

D 3
BTRowColumnWidget::BTRowColumnWidget(BTWidget *parent, char *const name,
                                     Dimension width, Dimension height)
E 3
I 3
BTRowColumnWidget::BTRowColumnWidget(BTWidget *parent, const char *name,
    Dimension width, Dimension height)
E 3
: BTWidget(parent)
{
D 3
  me_ = XtVaCreateWidget(name, xmRowColumnWidgetClass, parent ? parent->getWidget() : 0,
                         XmNwidth, width, XmNheight, height, NULL);
E 3
I 3
	me_ = XtVaCreateWidget(name, xmRowColumnWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNwidth, width,
	    XmNheight, height,
	    NULL);
E 3
}

D 3
BTRowColumnWidget::BTRowColumnWidget(BTWidget *parent, char *const name)
E 3
I 3
BTRowColumnWidget::BTRowColumnWidget(BTWidget *parent, const char *name)
E 3
: BTWidget(parent)
{
D 3
  me_ = XtVaCreateWidget(name, xmRowColumnWidgetClass, parent ? parent->getWidget() : 0,
                         XmNrowColumnType, XmWORK_AREA,
                         XmNpacking, XmPACK_COLUMN, XmNorientation, XmVERTICAL,
                         XmNnumColumns, 1, NULL);
E 3
I 3
	me_ = XtVaCreateWidget(name, xmRowColumnWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNrowColumnType, XmWORK_AREA,
	    XmNpacking, XmPACK_COLUMN,
	    XmNorientation, XmVERTICAL,
	    XmNnumColumns, 1,
	    NULL);
E 3
}
E 1
