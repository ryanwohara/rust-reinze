CREATE TABLE `rsn`
(
    `rsn`         char(12) CHARACTER SET latin1 COLLATE latin1_swedish_ci     NOT NULL,
    `host`        varchar(100) CHARACTER SET latin1 COLLATE latin1_swedish_ci NOT NULL DEFAULT '',
    `private`     tinyint(1) NOT NULL,
    `rsn_ident`   smallint                                                    NOT NULL DEFAULT '0',
    `inserted_on` timestamp                                                   NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_on`  timestamp                                                   NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    `id`          int unsigned NOT NULL AUTO_INCREMENT,
    PRIMARY KEY (`id`)
) ENGINE=InnoDB AUTO_INCREMENT=437 DEFAULT CHARSET=utf8mb3